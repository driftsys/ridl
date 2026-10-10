# ADR-0025 — Workspace-aware MCP tools: stateless snapshots, overlays in the loader, lookups over the checked IR, and the tool surface as a contract

## Status

Accepted — 2026-10-04. Scope: how the `ridl mcp` server reads a workspace, which
tools it offers, how it applies unsaved text, and what it promises about its own
surface. It binds every later change to a tool of `crates/ridl-mcp`, and every
later tool.

Written from piece 1a of the devex and agent tracks brief, implemented in
driftsys/ridl#668 (merge `edeec6e`) and #677 (merge `a87599a`). Sebastien agreed
decisions 1 to 6 in the brainstorming session of 2026-10-03; the design numbers
them D-1 to D-6, and **design D-n is decision n** for n from 1 to 6. Decision 7
comes from the reviewed design (§5.1). Decision 8 is the design's §4.4
amendment, which Sebastien approved on 2026-10-03 after the first
implementation. Decision 9 is the design's §7.2. The original design and plans
are
[`docs/archive/2026-10-03-mcp-workspace-tools-design.md`](../archive/2026-10-03-mcp-workspace-tools-design.md),
[`docs/archive/2026-10-03-mcp-workspace-tools-plan.md`](../archive/2026-10-03-mcp-workspace-tools-plan.md)
and
[`docs/archive/2026-10-03-mcp-tools-followup-plan.md`](../archive/2026-10-03-mcp-tools-followup-plan.md);
the design's §10 holds the alternatives, restated below.

It amends [ADR-0005](ADR-0005-agent-enablement.md) in place, in the same change:
the minimum tool table of §3 and the contract surfaces of §7. The as-built tools
are described in
[the MCP workspace tools design record](../design/mcp-workspace-tools.md); the
inputs and outputs of each tool are in
[the `ridl-mcp` README](../../crates/ridl-mcp/README.md).

Amended 2026-10-04 by [ADR-0026](ADR-0026-doc-comments.md) (documentation in the
source, issue #529): root discovery now walks from a workspace member to its
workspace for every entry point (ADR-0002 §4). Decision 2 is edited in place to
that rule: the note for a member path is gone, and the path mode reports only
the diagnostics of files under the member.

Amended 2026-10-06 by
[ADR-0027](ADR-0027-design-lints-calibrated-on-a-corpus.md) (workspace design
lints): a ninth tool, `ridl_metrics`, is added under decision 9, and the package
dependency edges that decision 8 counts are now computed in `ridlc::deps`, which
`ridl_metrics` reads. `ridl_dependencies` still reports the complete graph.

Amended 2026-10-10 (driftsys/ridl#786): `ridlc` no longer depends on
`ridl-diff`. The snapshot loader that `load_diff_side` calls moved to `ridl-ir`
as `ridl_ir::v2::load_ir_json`, under ADR-0008 decision 9. The consequence below
that says `ridlc` depends on `ridl-diff` records the state on 2026-10-04.

Amended 2026-10-09 (driftsys/ridl#770): decision 7 accepts an overlay path that
ends in `.rxdl` or `.rmdl`. In a package directory the loader reports such an
overlay with the warning RIDL-417 (`unsupported-source-file`), as it reports the
same file on disk, and does not compile it. A lone `.rxdl` or `.rmdl` file in
single-file mode, with or without an overlay of it, is a load error that names
the extension, and reaches the agent as a tool error.

## Context

Before this work `ridl mcp` exposed one tool, `ridl_check(source, profile)`. It
checked one pasted source against `ridl.std` only: an import of any other
package did not resolve, and every span named a synthetic file. An assistant
that works on a real workspace could not check the workspace on disk, check an
edit before writing it, look up a declaration that exists, see what depends on
it, see what a change breaks, or read what a diagnostic code means. ADR-0005 §3
names the minimum tool set; only the first tool existed, and only in its
pasted-source form.

## Decision

1. **Each tool call loads and checks the workspace from disk, keeps no state,
   and goes through one function** (design D-1). That function is `snapshot` in
   `crates/ridl-mcp/src/snapshot.rs`: it builds a `RidlDatabase`, compiles the
   workspace with the overlays, and returns the database, the checked output,
   the root and the notes. Every tool that takes `path` reads the disk only
   there, and `ridl_diff` loads its two sides through `ridlc::load_diff_side`. A
   cache can be added behind `snapshot` without a schema change. Measured on
   2026-10-03, `ridl check examples/cabin` takes less than 10 ms. **A real
   workspace whose snapshot takes more than 500 ms reopens this decision.**
2. **Root discovery is the command line's** (design D-2; amended by ADR-0026,
   see Status). A `path` resolves to a workspace exactly as `ridl check <path>`
   resolves it, through `ridl_core::find_root` (ADR-0002 §4). A path inside a
   workspace member loads the workspace whose `members` names the member, so
   imports of sibling members resolve and the root's `[lints]` apply.
   `ridl_check` in path mode then reports only the diagnostics of files under
   the member, and `workspace.errors` and `workspace.warnings` count only those.
   No note is added for a member path. As designed, the tools loaded the member
   alone and reported that in `workspace.notes` (driftsys/ridl#529); ADR-0026
   fixed the loader for every entry point together.
3. **The lookup tools read the checked IR, not the language server** (design
   D-3). The lookup logic is a `query` module in `ridl-mcp` over `ridlc`'s
   `WorkspaceOutput`, and it returns structured JSON. `ridl-lsp` is not changed
   and is not a dependency of `ridl-mcp`.
4. **`ridl_explain` answers from the existing catalogues and carries no document
   URL** (design D-4). A diagnostic code is looked up in
   `ridl_core::diag::ALL_CATALOGS` and returns its code, severity and one-line
   summary; a diff category word is looked up through `ridl_diff` and returns
   its explanation text. The mapping from each namespace to its explaining
   document is recorded in
   [the authoring skill outline](../wip/skill-ridl-authoring-outline.md) §7, for
   the authoring skill of piece 1c, which does not exist yet. A skill is prose
   that changes together with the documents, and a URL in a tool result is a
   contract that a moved chapter breaks. The long-form error index (E4.2) is not
   written here; when it lands it is added as an `explanation` field.
5. **One `ridl_check` with two modes, additive** (design D-5). The input
   `{source, profile}` keeps its meaning and its result `{diagnostics}`. The
   input `{path, overlays}` is added beside it and returns
   `{diagnostics, workspace}`. Exactly one mode is given; both, neither, a
   source without a profile, a path with a profile, and a source with overlays
   are tool errors. Path mode reports the diagnostics of the overlaid workspace,
   and differs from `ridl check <path>` only in what needs the network, the
   lockfile round trip or the baseline (the list is in the crate README). Path
   mode applies the project's `[lints]` levels, as ADR-0024 decision 8 requires.
6. **The tools are read-only and offline** (design D-6). No tool writes a file,
   and no tool fetches a remote import; a remote import produces the same Info
   diagnostic as `ridl check`. A test compares the fixture tree's files, sizes
   and modification times before and after a call of each tool.
7. **Unsaved text is applied inside the `ridl-core` loader, not after it**
   (design §5.1). `ridl_core::load_workspace_with` takes
   `Overlay { path, text }` values; `ridlc::compile_workspace_with` runs the
   same passes as `compile_workspace` over them. The loader enforces the
   package-and-directory law (TYPL-001, TYPL-002, TYPL-010) and interns each
   file's text for diagnostic spans while it reads the file, so an overlay
   applied afterwards would skip those checks and leave spans pointing at the
   text on disk. An overlay path must end in `.typl`, `.ridl`, `.rsdl`, `.rxdl`
   or `.rmdl`, and its directory must be a package directory of the loaded
   workspace; a new file joins the package of its directory as the same file
   saved to disk would. A `.rxdl` or `.rmdl` overlay is accepted so that the
   loader treats it as it treats the same file on disk: in a package directory
   it draws the warning RIDL-417 (`unsupported-source-file`) and is not
   compiled. A lone `.rxdl` or `.rmdl` file in single-file mode is a load error
   that names the extension, whether or not an overlay replaces its text.
   Anything else is a `LoadError` and reaches the agent as a tool error.
8. **`ridl_references` and `ridl_dependencies` count rsdl component uses**
   (design §4.4, approved 2026-10-03). Components exist only in the lowered
   system (`WorkspaceOutput.system`), not in the package IR. Each named,
   non-inline `Require` of a declared component is a reference to the required
   interface, reported with `kind: "component"`, a null interaction and a null
   location. A component's package depends on the catalog of each interface it
   requires, and the system's package depends on the packages of the components
   its member lines name. When the workspace has `.rsdl` files and no system was
   lowered, every workspace result carries a note that rsdl uses were not
   counted.
9. **The tool surface is the fourth agent contract surface of ADR-0005 §7**
   (design §7.2). Tool names, input schemas and output schemas change only by
   addition: a new optional input, a new output field, a new tool. A test pins
   the whole `tools/list` response against `crates/ridl-mcp/tests/tools.json`,
   so any schema change appears as a diff in review, and the test tells the
   reader to update that file deliberately when the change is additive.
   `ridl_metrics` is the first tool added under this rule (ADR-0027 decision 9).

## Alternatives considered

From the design's §10. The numbers are the decisions that reject them.

- **A cached session per workspace root, as the language server does.** The
  language server's session logic is private to its server state, and
  `load_workspace` creates new salsa inputs on each call, so a cache means a
  second copy of that bookkeeping (changed, new and deleted files, manifest
  edits) before a measurement shows it is needed. The `snapshot` seam keeps the
  option open. Rejected for decision 1.
- **An explicit `ridl_open` session handle.** It adds a step that models often
  get wrong, and its only benefit is the cache of decision 1. Rejected for
  decision 1.
- **`ridl-mcp` depends on `ridl-lsp` and calls its `nav` and `hover`.** Those
  functions are keyed by cursor offset and return Markdown, most of the
  rendering is private, and the dependency brings `lsp-server` and `lsp-types`
  into the MCP crate. Rejected for decision 3.
- **A shared crate (`ridl-query`) for the language server and the MCP server.**
  Deferred, not rejected: it changes `ridl-lsp` with no user benefit in this
  piece, and becomes the right move when the language server needs lookups by
  name. Decision 3.
- **Write the long-form error index inside this piece.** It is writing across
  the 148 catalogued codes, not wrapping compiler functions. Rejected for
  decision 4.
- **A `reference` URL for each namespace in the `ridl_explain` result.** A
  chapter that moves breaks the URL, which would be a tool contract under
  decision 9, and the FORM and MANI codes have no book chapter at all. Rejected
  for decision 4.
- **Explain with an excerpt of the reference prose.** The excerpt boundaries are
  guesses, and the build would depend on the Markdown in `docs/`. Rejected for
  decision 4.
- **Path mode only for `ridl_check`.** It breaks the documented contract and
  removes checking a draft before a workspace exists. Rejected for decision 5.
- **A separate `ridl_check_path` tool.** Two tools that do the same work, which
  models choose between badly. Rejected for decision 5.
- **Walk up to the enclosing workspace in the MCP only.** The MCP server and the
  command line would then disagree about the same path. Rejected for decision 2.
- **Apply overlays to the loaded workspace after the load.** Rejected for
  decision 7, for the reason given there.

## Consequences

- A tool that takes `path` costs one cold compile of the workspace for each
  call. This is accepted until decision 1's measurement condition is met.
- An agent that passes a file inside a workspace member checks the member inside
  its workspace, and sees the diagnostics of the member's files only (decision
  2, as ADR-0026 amends it).
- `ridl_references` and `ridl_dependencies` see rsdl uses only for the
  components that the workspace's `system` lists, which is also the set
  `ridl diff` and the IR dump see. A workspace with no lowered system gets the
  note of decision 8.
- Every added tool, input or output field is permanent (decision 9). A removal
  or a rename needs this record amended first.
- `ridlc` gains `compile_workspace_with`, `load_diff_side` and
  `WorkspaceOutput.imports`, and depends on `ridl-diff`; `ridl-core` gains
  `load_workspace_with`. `compile_workspace`, `load_workspace` and the CLI's
  `ridl diff` output and exit codes are unchanged.

## References

- [ADR-0005](ADR-0005-agent-enablement.md) §3 and §7 — the tool set and the
  contract surfaces; amended.
- [ADR-0024](ADR-0024-lint-registry-and-levels.md) decision 8 — which entry
  points apply lint levels; `ridl_check` in path mode does, and the other tools
  do not.
- [ADR-0010](ADR-0010-cli-conventions.md) — the CLI conventions that `ridl diff`
  keeps through the move of its loader into `ridlc`.
- [ADR-0026](ADR-0026-doc-comments.md) decision 10 — root discovery from a
  workspace member; amends decision 2.
- [The MCP workspace tools design record](../design/mcp-workspace-tools.md) and
  [the `ridl-mcp` README](../../crates/ridl-mcp/README.md).
