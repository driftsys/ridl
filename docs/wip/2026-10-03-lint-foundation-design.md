# Lint foundation — design for spec 0

Status: design spec for Spec 0 of
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md),
written 2026-10-03 against `main` at 440dfb59. Sebastien agreed the approach in
the brainstorming session of 2026-10-03 (decisions D-1 to D-7, §2). Nothing here
is implemented. It is archived with its plan when the work lands.

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
  commits the levels to the manifest, so the editor, the CLI and MCP always
  agree.
- **D-4. The levels are `allow`, `info`, `warn` and `deny`.** `allow` removes
  the diagnostic. `deny` emits it with Error severity, so ADR-0010's existing
  rule ("a diagnostic error over the checked source" exits 1) applies with no
  new exit code. There is no `forbid`: without a source attribute there is
  nothing for it to forbid.
- **D-5. The SARIF output carries no `helpUri` yet.** The long-form error index
  (ROADMAP E4.2) is not written, and 1a's `ridl_explain` serves the catalogue
  offline. A `helpUri` is added when an index page exists.
- **D-6. The registry and the step that applies levels live in `ridl-core`.**
  The catalogue rows carry the lint names, the manifest parser reads `[lints]`,
  and one function applies the levels. `ridlc` and `ridl-lsp` each call that
  function once (approach A, §10).
- **D-7. The lint names in §4.2 are agreed.** A name is a stable contract once
  it is released, because a `ridl.toml` refers to it.

## 3. Scope

In scope:

- A lint name on every Warning and Info catalogue row, and the guard test that
  keeps the names well formed (§4).
- The `[lints]` table in both manifest kinds, its resolution order, and the new
  code MANI-010 (§5).
- `apply_lint_levels` in `ridl-core` and its two call sites (§6).
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

A lookup `diag::lint_by_name(&str) -> Option<&'static CatalogEntry>` and
`diag::lint_of(DiagCode) -> Option<&'static str>` serve the manifest parser,
`apply_lint_levels` and the output formats.

A released lint name is never renamed or reused. A lint whose code is retired
keeps its name reserved, in the same way `RETIRED_RIDL_CODES` reserves codes.
This spec adds no alias mechanism.

### 4.2 The names

| Code     | Lint name                      | Default | Summary (from the catalogue)                                  |
| -------- | ------------------------------ | ------- | ------------------------------------------------------------- |
| TYPL-007 | `unused-import`                | warn    | unused import                                                 |
| TYPL-008 | `unneeded-import-alias`        | warn    | import alias without an actual collision                      |
| TYPL-101 | `unbounded-integer`            | warn    | `integer` without a range constraint                          |
| TYPL-102 | `unbounded-float`              | warn    | `float` without both a range and a `step`                     |
| TYPL-103 | `unbounded-length`             | warn    | `string`/`bytes` without explicit bounds                      |
| TYPL-115 | `no-init-value`                | info    | type has no derivable init value and no declared `= value`    |
| TYPL-211 | `duplicate-reserved`           | warn    | duplicate `reserved` entry                                    |
| TYPL-404 | `detached-doc-comment`         | warn    | blank line between a doc comment and its definition           |
| TYPL-405 | `deprecated-without-reason`    | warn    | `@deprecated` doc tag without a reason string                 |
| RIDL-100 | `missing-timing`               | warn    | `signal` or `event` without a timing annotation               |
| RIDL-108 | `degenerate-timing-range`      | warn    | degenerate timing range `@[X..X]`                             |
| RIDL-112 | `missing-response-bound`       | warn    | `command` or `query` with no declared response bound          |
| RIDL-304 | `error-typed-parameter`        | warn    | `error`-typed or result-union parameter on a command or query |
| RIDL-305 | `ensure-without-result`        | warn    | `ensure` clause that never references `result`                |
| RIDL-307 | `contract-error-name-in-enum`  | warn    | contract-error category name declared in an `error` enum      |
| RIDL-308 | `named-result-union-in-query`  | warn    | named result union in query return position                   |
| RIDL-404 | `query-named-like-mutation`    | warn    | query named like a mutation                                   |
| RIDL-405 | `shared-error-type`            | info    | one `error` type shared across unrelated failure domains      |
| RIDL-406 | `redeclared-envelope-metadata` | info    | payload struct re-declares envelope metadata                  |
| RIDL-407 | `ordinal-changed`              | warn    | ordinal changed against the published baseline                |
| RSDL-409 | `redundant-provider-set`       | warn    | a `requires` resolves to a redundant provider set             |
| RSDL-804 | `unclaimed-backend-key`        | warn    | a backend key whose namespace no configured backend claims    |
| MANI-005 | `unknown-manifest-key`         | warn    | unknown manifest key                                          |
| MANI-010 | `unknown-lint`                 | warn    | `[lints]` entry names no lint, or its value is not a level    |

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

`Manifest` gains a field `lints: LintTable`, a map from lint name to level. The
keys are kept as written; resolution against the registry happens in §5.3.

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
own. (Whether `ridl check` reports diagnostics in imported files today is
confirmed during planning; the rule holds either way.)

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
MANI-010 once and the whole entry is ignored.

## 6. Applying the levels

### 6.1 The function

```rust
pub fn apply_lint_levels(
    diagnostics: &mut Vec<Diagnostic>,
    levels_for: impl Fn(FileId) -> &LintLevels,
)
```

For each diagnostic whose code has a lint name, it reads the effective level
from `levels_for(primary.file)`:

- `allow` removes the diagnostic;
- `info`, `warn` and `deny` set its severity to Info, Warning and Error.

A diagnostic whose code is an Error code, and a diagnostic with no code
(`DiagCode::NONE`, such as the lockfile write failure in `ridlc`), is left
unchanged.

Because the function sets the severity of every lint diagnostic, the severity an
emit site chose no longer matters for a lint code. The emit sites that hard-code
a severity are not changed by this spec.

### 6.2 The call sites

- **`ridlc`.** Each public function that returns diagnostics to a caller
  (`check_source`, `compile`, `compile_workspace`, `run_check`, `run_build`,
  `run_build_with`, and on the 1a branch `compile_workspace_with`) calls
  `apply_lint_levels` once, just before it returns. The CLI (`ridl check`,
  `ridl build`), the `--baseline` path and the MCP server all receive the
  result. Whichever of 1a and this spec lands second adds the call to the
  function the other introduced.
- **`crates/ridl`, the `--baseline` path.** RIDL-407 is emitted by the CLI
  itself, after `ridlc` returns, so that path calls `apply_lint_levels` once
  more over the diagnostics it adds.
- **`ridl-lsp`.** `analyze` in `crates/ridl-lsp/src/server.rs` calls
  `apply_lint_levels` once, after the loader and checker diagnostics are
  gathered and before `convert::diagnostic`. The language server already loads
  the manifests through `ridl-core`, so the levels are available there. The
  existing severity mapping in `crates/ridl-lsp/src/convert.rs` needs no change.

`ridl build` stops when a lint at `deny` fires, because that diagnostic is now
an Error.

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

A lint diagnostic gets one extra note line after its labels and fix-its:

```text
= note: lint `missing-timing` (set its level in `[lints]` in ridl.toml)
```

### 7.3 SARIF

`ridl check --format sarif` writes one SARIF 2.1.0 log to stdout. `CheckFormat`
gains a `Sarif` variant. The projection is a small set of `serde` structs beside
`to_json` in `crates/ridl-core/src/diag.rs`, so the MCP server can reuse it
later; no SARIF crate is added.

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
  and column).
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
change. Path mode gets the project's levels through `ridlc`; source mode gets
the defaults (§5.2). After 1a has merged, `ridl_explain` adds the lint name and
the default level to its answer for a lint code.

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
  - `ridl build` fails on a lint at `deny`.
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
- **ADR-0010 decision 1**, amended in place with one sentence: a lint at `deny`
  is a diagnostic error, so it exits 1.
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
- **A SARIF crate** (`serde-sarif`). The subset used here is a few structs; a
  dependency is not worth it.
