# ridl-mcp

The RIDL MCP server (ADR-0005 Layer B): `ridl mcp` serves the Model Context
Protocol over stdio with eight read-only tools. It is a thin consumer of the
shared compiler crates — the same parser, resolver, and checker `ridl check` and
`ridl lsp` use.

## Install

`ridl` must be on `PATH`:

```sh
cargo install --path crates/ridl
```

Installer scripts, `install.sh` and `install.ps1`, exist at the repository root
and download the binary from the newest `editor-v*` GitHub Release.

## Tools

Every tool returns structured content with an output schema, plus the same JSON
in a text content block. Wrong requests return `isError: true` with an
actionable message. A compiler panic returns an MCP internal error.

Pass the workspace root (the directory that holds its `ridl.toml`) as `path`. A
package directory or source file is also accepted. Relative paths resolve
against the server's working directory. With a package manifest beneath a
workspace manifest, the member is loaded alone: sibling imports do not resolve
(driftsys/ridl#529). `workspace.notes` names the workspace root to pass instead.

Optional `overlays: [{path, source}]` replace unsaved text inside the loader.
Paths match by canonical parent plus file name. Only `.typl`, `.ridl` and
`.rsdl` files are accepted. A new file is accepted in an existing loaded package
directory; create its directory first. Empty text is valid. An overlay outside
the loaded workspace, in a hidden directory or separate package root, or for
another file in single-file mode is a tool error. Tools never write files or
fetch remote imports.

Workspace results carry `workspace: {root, errors, warnings, notes}`. Locations
are `{path, start, end}` with 1-based `{line, column}` and an exclusive end. The
range is the declared name. Fields and interactions use their enclosing
declaration's location; services and standard declarations have null locations.

### `ridl_check`

Input: either `path` and optional `overlays`, or `source` with `profile`
(`typl`, `ridl`, `rsdl`). Both or neither mode, a missing source profile, or a
profile with a path is a tool error.

Result: `{diagnostics, workspace}` in path mode, `{diagnostics}` in source mode.
Diagnostics use `ridl_core::diag::to_json`: `code`, `severity`, `lint`,
`message`, `span`, `labels` and `fixes`. `lint` is the lint name, present only
when the code is a lint. Path mode applies the workspace's `[lints]` levels
before the diagnostics are serialized; source mode gives every lint its default
level. A span carries `path`, `start` and `end`; labels carry `message` and
`span`; fixes carry `label`, `replacement` and `span`. A workspace with
diagnostics is a successful tool result.

**What this tool shares with `ridl check --format json <file>`, and where it
differs.** The CLI prints a bare diagnostic array; the tool wraps that array.
Source mode checks one standalone file against embedded `ridl.std`, including
the service catalogue and rsdl system checks. Its spans use `input.typl`,
`input.ridl` or `input.rsdl`. For a standalone file without a manifest or
imports, source mode agrees with the CLI except for those paths and the wrapper.
Path mode reports the same diagnostic objects as the CLI for the overlaid tree,
except for CLI operations that need the network, lockfile round trip or
baseline:

- MANI-101 to MANI-104 fetch and lock failures, and lockfile read and write
  diagnostics from `materialize_and_lock`;
- the check against `.ridl/baseline`: RIDL-407 and the rename hint on RIDL-409.
  Use `ridl_diff` with `old: "<root>/.ridl/baseline"` and `new: "<root>"` to
  compare the baseline explicitly.

Remote imports produce the same Info diagnostic in both faces: the import is not
yet available. Path loading and overlay failures are tool errors.

### `ridl_explain`

Input: `code`, an exact case-sensitive diagnostic code such as `TYPL-002` or
diff category word such as `payload_changed`. No workspace path is needed.

Result: `{kind: "diagnostic", code, severity, summary, lint, default_level}`
from the binary's catalogue, where `lint` is the lint name and `default_level`
its default level (`warn` or `info`), both present only when the code is a lint,
or `{kind: "diff_category", category, text}` from `ridl_diff::explain`. Unknown
inputs are tool errors naming FORM-, TYPL-, RIDL-, RSDL-, MANI- and saying that
diff category words are also accepted.

### `ridl_resolve`

Input: `path`, optional `overlays`, `name` and optional `from` package. A
canonical `pkg.Name` selects directly. A bare name searches workspace packages
and `ridl.std`; with `from` it resolves through that package's symbols,
including import aliases.

Result: `{name, package, kind, visibility, location, workspace}`, plus `alias`
when an import alias was used. Kinds are `type`, `const`, `struct`, `enum`,
`enumset`, `union`, `interface`; visibility is `public` or `internal`.

No match is a tool error with up to ten sorted canonical suggestions containing
the input, ignoring case. Ambiguity lists every match and directs the caller to
`pkg.Name` or `from`. An unknown package or unavailable checked IR is a tool
error; the latter directs the caller to `ridl_check` on the same path.

### `ridl_describe_type`

Input: `path`, optional `overlays`, `name`, optional `from`, resolved as above.
Result: `{declaration, package, location, workspace}`. `declaration` is a
complete non-interface IR `Decl` in canonical protobuf JSON, as in `.ir.json`
snapshots. Lookup errors are the same as `ridl_resolve`; an interface is a tool
error naming `ridl_list_interactions`.

### `ridl_list_interactions`

Input: `path`, optional `overlays`, `interface`, optional `from`. Result:
`{interface, interactions, location, workspace}`. The interface header carries
`name`, `package`, `doc`, `labels`, `deprecated`, `number`, `provisional`.
Interactions are canonical IR declarations in source order: `signal`, `event`,
`command`, `query`, `fixed`, or a `reserved` tombstone. Payloads, returns,
parameters, timing, ordinals and contracts retain the IR's fields. Lookup errors
are the same as `ridl_resolve`; a non-interface is a tool error.

### `ridl_references`

Input: `path`, optional `overlays`, `name`, optional `from`. Result:
`{target, references, workspace}` with canonical `target` and each reference
`{package, declaration, kind, interaction, location}`. `kind` is `declaration`,
`interface`, `service` or `component`. The walk includes nested type references,
pattern constants, enumset backing enums, interface references and inline
service interactions. Standard declarations are not walked. References are
reported once per `(declaration, interaction)` pair. Repeated fields without an
interaction yield one result; two interactions using the same target yield two
results. Service references use the service name and a null location. Lookup
errors are the same as `ridl_resolve`.

Declared components in the lowered rsdl system count as references when they
require a named interface. Inline requirements are covered by their service;
offers, links, routes and implicit components do not add references. Component
references have null interaction and location. If rsdl files were read but no
system was lowered, workspace notes say that rsdl uses were not counted and
recommend `ridl_check` on the same path.

### `ridl_dependencies`

Input: `path`, optional `overlays`, optional `package`. Result:
`{packages, workspace}`. Each package carries `name`, `imports` (manifest import
names from the member manifest's `[imports]` only, excluding the workspace
root's shared `[imports]`), sorted `depends_on` (other IR-referenced packages,
excluding `ridl.std`) and sorted `dependents` (workspace packages referencing
it). A `package` filter returns one package. An unknown package is a tool error
listing the available packages. This tool reports the graph; it does not lint
cycles or unused imports. Required named interfaces add dependencies from each
declared component's package to the interface's catalog. The system's package
also depends on the packages of its declared member components. The graph keeps
qualifiers that name unresolved or remote packages outside the workspace. When
the loaded workspace contains `.rsdl` files and no system was lowered, the
workspace carries the same no-system note as `ridl_references`.

### `ridl_diff`

Input: `old`, `new`, optional `overlays`. Each side accepts source paths,
`.ir.json` files or snapshot directories, using the CLI's loader. Overlays apply
to the new source side only and are refused for snapshots. Result: the object
`ridl diff --format json` prints, including `verdict` and `changes`, with the
standard IR supplied as context. Load failures are tool errors with the CLI's
message. A source side that does not compile returns `isError: true` with
structured `message` and `diagnostics`; the message identifies the old or new
side. The diagnostics retain their full JSON shape.

## Host configuration

### Claude Code

Add to the project's `.mcp.json` (or run `claude mcp add ridl -- ridl mcp`):

```json
{
  "mcpServers": {
    "ridl": { "command": "ridl", "args": ["mcp"] }
  }
}
```

### Codex

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.ridl]
command = "ridl"
args = ["mcp"]
```

### GitHub Copilot in VS Code

Installing the RIDL VS Code extension registers `ridl mcp` with VS Code
automatically, on VS Code 1.101 or newer, through
`vscode.lm.registerMcpServerDefinitionProvider`. No manual configuration is
needed.

A user who is not using the extension configures it manually instead: install
`ridl` on `PATH` as above, then add it as a server the same way the Claude Code
or Codex sections above do, in whichever place your version of VS Code keeps its
MCP server list.
