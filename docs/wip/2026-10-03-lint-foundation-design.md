# Lint foundation — design for spec 0

Status: design spec for Spec 0 of
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md),
written 2026-10-03 against `main` at 440dfb59. Sebastien agreed the approach in
the brainstorming session of 2026-10-03 (decisions D-1 to D-7, §2). The
maintainer's delegate took D-8 and D-9 in the pass-1 review of PR #671. Nothing
here is implemented. It is archived with its plan when the work lands.

Satisfies: the brief's Spec 0 (a lint registry, a `[lints]` table in
`ridl.toml`, `ridl check --format sarif`, and the same lint results through
`ridl-lsp` and `ridl-mcp`). Related: ADR-0002 §4 (the manifest), ADR-0010
decision 1 (exit codes), ADR-0005 §7 (the JSON diagnostic shape is an external
contract), #191 (diagnostic codes written in Markdown are unguarded).

## 1. The problem

The compiler already emits warnings and infos, but none of them can be
configured:

- `Severity` is `Error | Warning | Info` (`crates/ridl-core/src/diag.rs`).
  Twenty-three catalogue codes are Warning or Info (§4.2). The lint pass in
  `crates/ridl-sem/src/lint.rs` says on purpose that E2 has "no lint driver and
  no configuration surface".
- Each emit site chooses the severity. The catalogue's severity is a default
  only, and some sites hard-code their own (for example
  `crates/ridl-sem/src/resolve.rs` and RIDL-407 in `crates/ridl/src/main.rs`).
- `ridl check` exits 1 only when an Error is present. A warning never changes
  the exit code, and there is no flag that makes it do so.
- `ridl check --format` accepts `text` and `json`. There is no SARIF output, so
  a CI system that reads SARIF (GitHub code scanning, for example) cannot show
  RIDL findings.

Spec 2a adds doc lints (`missing-docs`, a broken `[Type]` link, and others), and
the brief decides that `missing-docs` is a warning by default that a project can
raise in `ridl.toml`. Spec 1b adds design lints with the same need. Both need a
level per lint, set per project, that gives the same result in the CLI, the
language server and the MCP server.

## 2. Decisions taken in the brainstorming session

- **D-1. Every Warning and Info catalogue code is a lint.** Each one gets a
  stable kebab-case lint name, and its catalogue severity becomes its default
  level. Error codes are never lints and can never be configured. There is one
  diagnostic channel and one catalogue; a lint is a catalogue row that has a
  name.
- **D-2. A level is set only in the `[lints]` table of a `ridl.toml`.** There is
  no source attribute (such as `@allow(...)`) in this spec. A source attribute
  stays a possible follow-up (§10).
- **D-3. The command line does not override levels.** There is no
  `--deny-warnings` and no `-A`/`-W`/`-D` flag. A CI that wants stricter checks
  commits the levels to the manifest, so the editor, the CLI and MCP agree for
  the same entry point. Entering at a workspace member is a different entry
  point (D-9).
- **D-4. The levels are `allow`, `info`, `warn` and `deny`.** `allow` removes
  the diagnostic. `deny` emits it with Error severity, so in `ridl check` and
  `ridl build` ADR-0010's existing rule ("a diagnostic error over the checked
  source" exits 1) applies with no new exit code (D-8). There is no `forbid`:
  without a source attribute there is nothing for it to forbid.
- **D-5. The SARIF output carries no `helpUri` yet.** The long-form error index
  (ROADMAP E4.2) is not written, and 1a's `ridl_explain` serves the catalogue
  offline. A `helpUri` is added when an index page exists.
- **D-6. The registry and the step that applies levels live in `ridl-core`.**
  The catalogue rows carry the lint names, the manifest parser reads `[lints]`,
  and one function applies the levels. The entry points that report diagnostics
  call that function (approach A, §10; the call sites are in §6.2).
- **D-7. The lint names in §4.2 are agreed.** A name is a stable contract once
  it is released, because a `ridl.toml` refers to it.
- **D-8. Levels apply only where diagnostics are reported to a person or an
  agent.** These are `ridl check` (including the `--baseline` desk check),
  `ridl build`, `ridlc check` and `ridlc build` (they go through `run_check` and
  `run_build_with`), the language server, and the MCP tool `ridl_check`. Every
  other command (`ridl diff`, `ridl test`, `ridl baseline`, `ridl lock`, the MCP
  tool `ridl_diff`, and the MCP lookup tools) compiles with the severities the
  emit sites chose, so a lint at `deny` never makes one of them fail or exit 2.
  A `[lints]` table is a reporting setting; it does not change whether a
  workspace compiles.
- **D-9. Entering at a workspace member loads the member alone.**
  `ridl check <member>`, `ridl check` on a file inside a member, MCP path mode
  on a member, and an editor opened on a member load the member as a standalone
  package, so the workspace root's `[lints]` does not apply. This is how
  `[defaults].timing` and `[imports]` behave today, and it is part of the
  language server gap #529, which stays open. Spec 0 does not change the
  loader's root discovery.

## 3. Scope

In scope:

- A lint name on every Warning and Info catalogue row, and the guard test that
  keeps the names well formed (§4).
- The `[lints]` table in both manifest kinds, its resolution order, and the new
  code MANI-010 (§5).
- `apply_lint_levels` in `ridl-core` and its call sites in the entry points that
  report diagnostics (§6).
- The `lint` field in the JSON output, the lint note in the text output, and
  `ridl check --format sarif` (§7).
- The book page that lists the lints, with a test that compares it with the
  catalogue, and the CLI reference updates (§9).
- The amendments to ADR-0002 §4 and ADR-0010 decision 1 (§9).

Out of scope:

- Any new lint other than MANI-010. Spec 2a adds the doc lints, spec 1b the
  design lints.
- A source attribute that sets a level on one declaration (D-2).
- Command-line overrides (D-3).
- A `helpUri`, a long-form help text per code, or the error index (D-5).
- SARIF output for any subcommand other than `ridl check`.
- Applying levels in any command other than those D-8 names.
- A change to the loader's root discovery for a member entry point (D-9).
- Fix-its in the SARIF output (§7.3).

## 4. The registry

### 4.1 Lint names in the catalogue

Each row of the `diag_codes!` macro in `crates/ridl-core/src/diag.rs` gets an
optional lint name. `CatalogEntry` gains a field:

```rust
pub struct CatalogEntry {
    pub code: DiagCode,
    pub severity: Severity,
    pub summary: &'static str,
    /// The lint name, present exactly when `severity` is Warning or Info.
    pub lint: Option<&'static str>,
}
```

The catalogue severity of a lint row is its default level: Warning is `warn`,
Info is `info`.

A guard test beside `catalog_entries_are_well_formed_ordered_and_unique` checks,
over `ALL_CATALOGS`:

- every Warning or Info row has a lint name;
- no Error row has a lint name;
- every name matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`;
- no two rows, in any catalogue, share a name.

The lookups
`ridl_core::lint::lint_by_name(&str) -> Option<&'static CatalogEntry>` and
`ridl_core::lint::lint_of(DiagCode) -> Option<&'static CatalogEntry>` (only rows
that have a lint name) serve the manifest parser, `apply_lint_levels` and the
output formats.

A released lint name is never renamed or reused. A lint whose code is retired
keeps its name reserved, in the same way `RETIRED_RIDL_CODES` reserves codes.
This spec adds no alias mechanism.

### 4.2 The names

| Code     | Lint name                      | Default | Summary (from the catalogue)                                                           |
| -------- | ------------------------------ | ------- | -------------------------------------------------------------------------------------- |
| TYPL-007 | `unused-import`                | warn    | unused import                                                                          |
| TYPL-008 | `unneeded-import-alias`        | warn    | import alias without an actual collision                                               |
| TYPL-101 | `unbounded-integer`            | warn    | `integer` without a range constraint                                                   |
| TYPL-102 | `unbounded-float`              | warn    | `float` without both a range and a `step`                                              |
| TYPL-103 | `unbounded-length`             | warn    | `string`/`bytes` without explicit bounds                                               |
| TYPL-115 | `no-init-value`                | info    | type has no derivable init value and no declared `= value`                             |
| TYPL-211 | `duplicate-reserved`           | warn    | duplicate `reserved` entry                                                             |
| TYPL-404 | `detached-doc-comment`         | warn    | blank line between a doc comment and its definition                                    |
| TYPL-405 | `deprecated-without-reason`    | warn    | `@deprecated` doc tag without a reason string                                          |
| RIDL-100 | `missing-timing`               | warn    | `signal` or `event` without a timing annotation                                        |
| RIDL-108 | `degenerate-timing-range`      | warn    | degenerate timing range `@[X..X]`                                                      |
| RIDL-112 | `missing-response-bound`       | warn    | `command` or `query` with no declared response bound                                   |
| RIDL-304 | `error-typed-parameter`        | warn    | `error`-typed or result-union parameter on a `command` or `query`                      |
| RIDL-305 | `ensure-without-result`        | warn    | `ensure` clause that never references `result`                                         |
| RIDL-307 | `contract-error-name-in-enum`  | warn    | contract-error category name declared in an `error` enum                               |
| RIDL-308 | `named-result-union-in-query`  | warn    | named result union in query return position                                            |
| RIDL-404 | `query-named-like-mutation`    | warn    | query named like a mutation                                                            |
| RIDL-405 | `shared-error-type`            | info    | one `error` type shared across unrelated failure domains                               |
| RIDL-406 | `redeclared-envelope-metadata` | info    | payload struct re-declares envelope metadata                                           |
| RIDL-407 | `ordinal-changed`              | warn    | interaction, struct field, or union arm ordinal changed against the published baseline |
| RSDL-409 | `redundant-provider-set`       | warn    | a `requires` resolves to a redundant provider set                                      |
| RSDL-804 | `unclaimed-backend-key`        | warn    | a backend key whose namespace no configured backend claims                             |
| MANI-005 | `unknown-manifest-key`         | warn    | unknown manifest key                                                                   |
| MANI-010 | `unknown-lint`                 | warn    | `[lints]` entry names no lint, or its value is not a level                             |

MANI-010 is the only new code (§5.3).

TYPL-404 and TYPL-405 are already doc lints. Spec 2a uses them as they are; the
brief's "`@deprecated` without a reason" lint is TYPL-405.

Because of D-1, RIDL-407 (`ordinal-changed`), the warning that the
`ridl check --baseline` gate raises, can be set to `allow`. A project that does
so makes the baseline gate silent for ordinal changes. This is a consequence of
D-1 that the maintainer accepted; the book page states it next to the lint.

## 5. The `[lints]` table

### 5.1 Shape

```toml
[lints]
missing-timing = "deny"
shared-error-type = "allow"
```

A package manifest and a workspace manifest both accept a `[lints]` table. Each
key is a lint name; each value is one of the strings `allow`, `info`, `warn` or
`deny`. `lints` joins the list of known top-level keys in `check_unknown_keys`
(`crates/ridl-core/src/manifest.rs`), so it no longer raises MANI-005.

`Manifest` gains a field `lints: LintTable`, a map from lint name to level. Each
key is the registry's own name (`&'static str`): the parser keeps only entries
whose key is a registered lint name and whose value is a level, and raises
MANI-010 for every other entry (§5.3).

### 5.2 Resolution

The level of a lint for a given package is resolved key by key, the later source
winning:

1. the registry default (§4.2);
2. the `[lints]` table of the workspace root manifest, when the package is a
   workspace member;
3. the package's own `[lints]` table.

A package that is not a workspace member uses its own table over the defaults.
The result is a `LintLevels` value: the default levels with the overrides
applied.

The package that owns a diagnostic decides its level: the package whose
directory contains the file of the diagnostic's primary span. A file that no
workspace package owns (the standard library, a fetched remote import) uses the
registry defaults. No default is `deny`, so a dependency can never fail a
project's check, and a project's `[lints]` never applies to code it does not
own. Today `ridl check` reports no diagnostic in a file that a workspace package
does not own: no package is loaded as a remote package (`PackageOrigin::Remote`
is never constructed outside tests), remote imports are only fetched and pinned
by `materialize_and_lock` in `ridlc` after the check, which returns manifest and
lockfile diagnostics only, and the diagnostics of `ridl.std` are not merged into
the workspace output (`WorkspaceOutput::std_ir`). The rule holds for any such
file a later change adds.

The levels follow the entry point (D-9). When the entry is a workspace member,
or a file inside one, the loader stops at the member's own `ridl.toml` and loads
the member as a standalone package, so step 2 above does not apply and the root
`[lints]` is ignored. The same is true of `[defaults].timing` and `[imports]`
today.

When there is no manifest at all — `ridl check` on a single file outside any
package, and `ridl_check` in its `{source, profile}` mode — the registry
defaults apply.

### 5.3 Errors in `[lints]`

The table never stops a check. Each of these raises MANI-010 (`unknown-lint`,
Warning) with its primary span on the offending key, and the entry is ignored:

- the key is not a lint name in the registry (this includes the name of an Error
  code, which has none, and a code such as `RIDL-100` written as a key);
- the value is not a string;
- the value is a string other than `allow`, `info`, `warn` or `deny`.

The message names the problem. MANI-010 is itself a lint, and the levels are
applied after all diagnostics are produced (§6), so a `[lints]` table can set
the level of the MANI-010 diagnostics it causes itself.

`[lints]` must be a table. A `lints` key with any other TOML type raises
MANI-010 once, with its primary span on the value, and the whole entry is
ignored. The rest of the manifest is still read.

## 6. Applying the levels

### 6.1 The function

```rust
pub fn apply_lint_levels(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    scopes: &LintScopes,
)
```

`LintScopes` maps directories to effective levels (`LintLevels`). The loader
builds it: one scope for the workspace root directory (the defaults overlaid
with the root table), one per member directory (the root levels overlaid with
the member table), one for a standalone package's directory, and none in
single-file mode. The levels resolve by directory and not by file id, because a
file id does not always survive: the CLI's RIDL-407 diagnostics get spans
interned into the source map after `ridlc` returns, and a file created in the
editor after the workspace loaded has no entry. The lookup takes the longest
directory, compared component by component, that is a prefix of
`sources.path(primary.file)`. A member's own `ridl.toml` and every file under
the member directory get the member's levels; the root `ridl.toml` gets the
root's levels. A diagnostic whose file has no path, or whose path is in no
scope, gets the registry defaults.

For each diagnostic whose code has a lint name, it reads the effective level
from that lookup:

- `allow` removes the diagnostic;
- `info`, `warn` and `deny` set its severity to Info, Warning and Error.

A diagnostic whose code is an Error code, and a diagnostic with no code
(`DiagCode::NONE`, such as the lockfile write failure in `ridlc`), is left
unchanged.

Because the function sets the severity of every lint diagnostic, the severity an
emit site chose no longer matters for a lint code where levels are applied. The
emit sites that hard-code a severity are not changed by this spec, and the
commands that do not apply levels (D-8) keep those severities. The function is
idempotent.

### 6.2 The call sites

The shared compile in `ridlc` (`check_loaded`, and so `compile_workspace` and
`compile_workspace_with`) does not apply the levels (D-8). It carries the loaded
`LintScopes` out on its outputs: `Compiled` passes them to `CliRun.lints`, and
`WorkspaceOutput` gains a `lints` field. The entry points that report
diagnostics apply them:

- **`ridlc`.** `run_check` applies them before it returns. `run_build_with`
  applies them before its `succeeded` gate and before it writes any artifact,
  when its caller asks for it: `ridl build` and `ridlc build` ask, and
  `ridl baseline`, which calls the same function to publish its snapshot, does
  not (D-8). `check_source` and `compile` apply them with empty scopes, so the
  registry defaults apply.
- **`crates/ridl`, the `--baseline` path.** RIDL-407 is emitted by the CLI
  itself, after `ridlc` returns, so that path calls `apply_lint_levels` once
  more, with `CliRun.lints`, over the whole run after the desk check. The desk
  check runs today only when no error other than RIDL-409 is present
  (`only_lock_orphans` in `crates/ridl/src/lock.rs`); at the `ridl check` call
  site, diagnostics whose code has a lint name are left out of that test, so a
  lint at `deny` does not stop the desk check. `ridl lock`, which uses the same
  function, is unchanged.
- **`ridl-mcp`.** `ridl_check` in path mode applies `WorkspaceOutput.lints`
  after `compile_workspace_with` returns and before it builds the JSON and the
  workspace status counts. Source mode goes through `check_source`.
- **`ridl-lsp`.** `analyze` never sees the loader diagnostics: `load()` in
  `crates/ridl-lsp/src/server.rs` converts them itself. So `load()` keeps the
  loaded `LintScopes` and applies them to the loader diagnostics before it
  converts them, and `analyze` applies them to its own diagnostics before it
  converts them. The existing severity mapping in
  `crates/ridl-lsp/src/convert.rs` needs no change.

`ridl build` stops when a lint at `deny` fires, because that diagnostic is now
an Error. `ridl diff`, `ridl test`, `ridl baseline`, `ridl lock`, MCP
`ridl_diff` and the MCP lookup tools compile through `compile_workspace` or
`compile_workspace_with`, or through `run_build_with` without levels, so their
results do not change.

## 7. Outputs

### 7.1 JSON

Each element of the JSON array (`JsonDiagnostic`, `diag::to_json`) gains an
optional field:

```json
{ "code": "RIDL-100", "severity": "error", "lint": "missing-timing", ... }
```

`lint` is present when the code has a lint name and absent otherwise. The
`severity` is the effective one. Adding a field is a compatible change to the
contract ADR-0005 §7 describes. `docs/book/cli-reference.md` documents the
field.

### 7.2 Text

A lint diagnostic gets one extra note after its fix-it notes. The note string is
``lint: `<name>` (set its level in `[lints]` in ridl.toml)``.
`codespan-reporting` renders a note string as given, after a `=` bullet
(`crates/ridl-core/src/diag/render.rs`), in the same form as the existing fix-it
note `suggestion: replace with ...`. For a file whose line numbers have one
digit, the rendered line is two spaces of gutter followed by this text:

```text
= lint: `missing-timing` (set its level in `[lints]` in ridl.toml)
```

The note depends only on the code, so it also appears in the text output of the
commands that do not apply levels (D-8), such as `ridl test`.

### 7.3 SARIF

`ridl check --format sarif` writes one SARIF 2.1.0 log to stdout. `CheckFormat`
gains a `Sarif` variant. The projection is a small set of `serde` structs in
their own module, `crates/ridl-core/src/diag/sarif.rs`, next to `render.rs`, so
the MCP server can reuse it later; no SARIF crate is added.

- `version` is `"2.1.0"` and `$schema` names the 2.1.0 schema.
- One run. `tool.driver` has `name` `"ridl"`, `version` the crate version, and
  `rules` holding every row of `ALL_CATALOGS`, errors included, so a viewer can
  describe every result.
- Each rule: `id` is the code, `name` is the lint name when there is one,
  `shortDescription.text` is the catalogue summary, and
  `defaultConfiguration.level` is the default level mapped to SARIF (`warn` →
  `warning`, `info` → `note`, Error → `error`).
- Each result: `ruleId` and `ruleIndex` for a coded diagnostic (an uncoded
  diagnostic has neither), `level` the effective level mapped the same way,
  `message.text`, and one `location` with a `physicalLocation` (the artifact URI
  relative to the checked root, and a `region` with 1-based start and end line
  and column). A diagnostic whose primary span has no path in the source map
  (`FileId::DETACHED`, which the MANI-1xx manifest, lockfile and fetch
  diagnostics and the uncoded lockfile write warning carry) has no `locations`
  property.
- Each label becomes a `relatedLocation` with its message.
- `run.columnKind` is `"unicodeCodePoints"`, because RIDL columns count
  characters and SARIF's default unit is UTF-16 code units.
- Fix-its are not emitted. GitHub code scanning ignores them, and the JSON
  output already carries them.
- An `allow`ed diagnostic does not appear.

The exit code follows ADR-0010 decision 1, as for the other formats: 1 when an
Error (including a lint at `deny`) is present, else 0; 2 when the check could
not run.

### 7.4 MCP

`ridl_check` uses `to_json`, so it gains the `lint` field with no MCP-specific
change. Path mode applies the project's levels from `WorkspaceOutput.lints` in
its handler (§6.2); source mode gets the defaults through `check_source` (§5.2).
`ridl_explain` (`crates/ridl-mcp/src/explain.rs`, on `main` since 1a) adds two
optional fields to its answer for a lint code, `lint` (the lint name) and
`default_level` (`warn` or `info`); both are absent for an Error code. Adding
fields is a compatible change to the tool surface.

## 8. Testing

- **Catalogue guard** (§4.1): the four rules over `ALL_CATALOGS`.
- **Manifest unit tests** in `crates/ridl-core/src/manifest.rs`: `[lints]` in
  both manifest kinds; `lints` no longer raises MANI-005; each MANI-010 case in
  §5.3, with its span; the root-then-member resolution, including a key set at
  the root and overridden by the member.
- **`apply_lint_levels` unit tests**: each level on a lint code; an Error code
  and an uncoded diagnostic left unchanged; a diagnostic in a file no package
  owns uses the defaults even when the project sets `deny`.
- **CLI tests** in `crates/ridl/tests/`, over a fixture workspace with a root
  and a member `[lints]`:
  - a lint at `deny` makes `ridl check` exit 1 and shows as an error, in text,
    JSON and SARIF;
  - a lint at `allow` is absent from all three formats;
  - `ridl build` fails on a lint at `deny`;
  - `ridl diff` between two copies of a fixture that sets a lint to `deny` exits
    0 (D-8).
- **MCP tests**: `ridl_check` in path mode over a fixture that sets a lint to
  `deny` reports that diagnostic with severity `error` and a `lint` field;
  `ridl_explain` on a lint code returns its lint name and default level.
- **SARIF test**: the SARIF output for the fixture is validated against the
  official SARIF 2.1.0 JSON schema, vendored into the test fixtures and checked
  with the `jsonschema` dev-dependency, and compared with a snapshot.
- **Parity test**: the same fixture through `ridlc` and through `ridl-lsp`'s
  `analyze` gives the same codes and severities.
- **Book table test** (§9): `docs/book/lints.md` lists exactly the catalogue's
  lints with the right code, default and summary.

## 9. Documentation and records

- **New book page `docs/book/lints.md`**, linked from `SUMMARY.md`: what a lint
  is, the `[lints]` table and its resolution, and the table from §4.2. A test in
  `crates/ridl/tests/` parses the table and compares it with `ALL_CATALOGS`, so
  a lint added to the catalogue without a row in the book fails the build. This
  is a first, narrow check of diagnostic codes written in Markdown (#191).
- **`docs/book/cli-reference.md`**: `--format sarif`, the `lint` JSON field, and
  a pointer to the lints page.
- **ADR-0002 §4**, amended in place: the `[lints]` table, which manifest kinds
  accept it, and the resolution order of §5.2.
- **ADR-0010 decision 1**, amended in place with one sentence: in `ridl check`,
  `ridl build`, `ridlc check` and `ridlc build`, a lint at `deny` is a
  diagnostic error, so it exits 1; the other subcommands of both binaries do not
  apply lint levels (D-8).
- **`crates/ridl-sem/src/lint.rs`**: the header comment that says there is no
  configuration surface is updated to point at the registry.

## 10. Alternatives considered

- **Only new lints in the registry; existing warnings stay fixed.** Smaller
  today, but it leaves two kinds of warning that behave differently, which the
  book would have to explain. Rejected for D-1.
- **Every warning is a lint except a few hazards** (MANI-005, the lockfile write
  failure, RIDL-407). Rejected by the maintainer: one rule is easier to state.
  The uncoded lockfile warning stays fixed anyway, because it has no code.
- **A source attribute such as `@allow(missing-docs)`.** Gives a local exception
  without turning a lint off for the whole package, but changes the language
  surface (an ADR, checked against ADR-0011, ADR-0012 and ADR-0015). Deferred:
  added when real false positives from 2a or 1b show the need.
- **`--deny-warnings`, or cargo-style `-A`/`-W`/`-D` flags.** Convenient for CI,
  but the editor and CI could then disagree. Rejected for D-3.
- **A `helpUri` into the book, or a long-form help text per code now.** The
  first needs an anchor per code kept in sync, the second about 150 help texts.
  Deferred to the error index (D-5).
- **Pass the levels into `ridl-sem` so each emit site checks its level (approach
  B).** Skips the work for allowed lints but changes every emit site and pass
  signature, for a saving that does not matter at current sizes.
- **Apply the levels in each front end separately (approach C).** Touches the
  fewest files, but three copies drift, and the brief asks for the same results
  everywhere.
- **A `forbid` level.** Has nothing to forbid without a source attribute.
- **Apply the levels inside the shared compile, for every command.** Simpler,
  with one call in `check_loaded`, but a lint at `deny` would then make
  `ridl diff` and `ridl test` exit 2, block `ridl baseline` and `ridl lock`, and
  make MCP `ridl_diff` report that the workspace does not compile. Rejected for
  D-8: a lint level is a reporting setting.
- **Walk up from a member to its workspace root to find the root `[lints]`.**
  Would give a member entry point the same levels as a root entry point, but it
  changes the loader's root discovery, and with it how `[defaults].timing` and
  `[imports]` behave at a member, which belongs with the work on #529. Rejected
  for Spec 0 (D-9).
- **A SARIF crate** (`serde-sarif`). The subset used here is a few structs; a
  dependency is not worth it.
