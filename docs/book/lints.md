# Lints

A lint is a diagnostic that a project can turn off, report at a different
severity, or turn into an error. Every warning and every info diagnostic in the
catalogue is a lint: each has a stable kebab-case name, and its catalogue
severity is its default level, unless the table at the end of this page gives
its default as `allow`. A lint that is `allow` by default is reported only when
a project sets its level. An error diagnostic is never a lint, and its severity
cannot be changed.

The lint name appears in every report of a lint diagnostic. The JSON report of
[`ridl check --format json`](cli-reference.md#ridl-check) carries it in the
`lint` field, and the text report adds a note after the snippet:

```text
warning[TYPL-102]: `float` without both a range and a `step`
  ┌─ ./demo.ridl:3:15
  │
3 │ type Broken : km/h [250.0..0.0]
  │               ^^^^
  │
  = lint: `unbounded-float` (set its level in `[lints]` in ridl.toml)
```

## Levels

A lint has one of four levels:

- `allow` removes the diagnostic from the report.
- `info` reports the diagnostic as an info.
- `warn` reports the diagnostic as a warning.
- `deny` reports the diagnostic as an error. In `ridl check`, `ridl build`,
  `ridlc check` and `ridlc build`, an error makes the command exit 1, so a lint
  at `deny` fails the run the same way any other error does.

No lint defaults to `deny`.

## The `[lints]` table

A level is set in the `[lints]` table of a `ridl.toml`. Each key is a lint name
and each value is one of the strings `allow`, `info`, `warn` or `deny`:

```toml
[lints]
missing-timing = "deny"
shared-error-type = "allow"
```

A package manifest and a workspace manifest both accept the table. There is no
command-line flag that changes a level and no source attribute that changes a
level: a CI that wants stricter checks commits the levels to the manifest, so
the editor, the command line and an agent see the same levels for the same
entry point.

The level of a lint for a package is resolved key by key, and a later source
replaces an earlier one:

1. the default level from the table below;
2. the `[lints]` table of the workspace root manifest, when the package is a
   workspace member;
3. the package's own `[lints]` table.

A diagnostic takes the levels of the package whose directory contains the file
of its primary span. In a workspace, the root's `[lints]` table covers the
whole directory tree of the root: a file under the workspace root that no
member contains, and the root `ridl.toml` itself, take the root's levels. A
file outside the directory tree of the entry point (the standard library, a
fetched import, an editor buffer outside the project) uses the default levels,
so a project's `[lints]` never applies to code outside its directory. When there
is no manifest at all, for example `ridl check` on a single file outside any
package, the default levels apply.

**A member loads its workspace.** Running `ridl check` on a workspace member
or on a file inside a member, opening an editor on a member, or passing a
member as the `path` of the MCP tool `ridl_check` loads the workspace whose
`members` lists the member. The workspace root's `[lints]` table applies to the
member, and the member's own table is applied over it, as in a check from the
root. `[defaults].timing` and `[imports]` behave the same way. The command
reports only the diagnostics of files under the member; check from the
workspace root to see every member's diagnostics. A package that no workspace
lists stays standalone, and the search for a workspace stops at the first
`[workspace]` manifest and at a directory that holds `.git`.

**Known limitations of the language server.** The language server reads the
`[lints]` tables once, when it loads the workspace: at start, or when the first
file is opened. An edit to a `[lints]` table takes effect in the editor after
the server restarts. The language server publishes the diagnostics of every
loaded member, also when the editor is opened on one member.

## Where levels apply

Levels apply where diagnostics are reported to a person or an agent:

- `ridl check`, including the desk check against a baseline;
- `ridl build`, which stops before it writes any artifact when a lint is at
  `deny`;
- `ridlc check` and `ridlc build`;
- the language server;
- the MCP tool `ridl_check`.

Every other command reports the severities the emit sites chose: `ridl diff`,
`ridl test`, `ridl baseline`, `ridl lock`, the MCP tool `ridl_diff` and the MCP
lookup tools. A lint at `deny` never makes one of them fail. The text note that
names the lint still appears in their output. A lint that is `allow` by default
is left out of their output, as it is everywhere else.

## Mistakes in `[lints]`

A mistake in the table never stops a check. Each of these raises MANI-010
(`unknown-lint`, a warning) on the entry, and the entry is ignored:

- the key is not a lint name, which includes a diagnostic code written as a key
  (`RIDL-100 = "deny"`) and the name of an error code, which has none;
- the value is not a string;
- the value is a string other than `allow`, `info`, `warn` or `deny`, which
  includes a level with the wrong case (`"Deny"`);
- `lints` is not a table.

MANI-010 is itself a lint, so the table can set its level.

## The lints

| Lint | Code | Default | Summary |
| --- | --- | --- | --- |
| `unused-import` | TYPL-007 | warn | unused import |
| `unneeded-import-alias` | TYPL-008 | warn | import alias without an actual collision |
| `unbounded-integer` | TYPL-101 | warn | `integer` without a range constraint |
| `unbounded-float` | TYPL-102 | warn | `float` without both a range and a `step` |
| `unbounded-length` | TYPL-103 | warn | `string`/`bytes` without explicit bounds |
| `no-init-value` | TYPL-115 | info | type has no derivable init value and no declared `= value` |
| `duplicate-reserved` | TYPL-211 | warn | duplicate `reserved` entry |
| `inconsistent-unit` | TYPL-222 | info | one field name used with different units |
| `inconsistent-abbreviation` | TYPL-223 | info | inconsistent identifier abbreviation |
| `duplicate-shape` | TYPL-224 | info | duplicate declaration shape |
| `broken-doc-link` | TYPL-401 | warn | doc link or `@see` target that does not resolve |
| `detached-doc-comment` | TYPL-404 | warn | blank line between a doc comment and its carrier |
| `deprecated-without-reason` | TYPL-405 | warn | `@deprecated` doc tag without a reason string |
| `missing-docs` | TYPL-406 | warn | item without a doc comment |
| `misplaced-doc-comment` | TYPL-407 | warn | doc comment in a position that is not a carrier |
| `unknown-doc-tag` | TYPL-408 | warn | doc tag other than `@see`, `@since`, `@deprecated` and `@labels` |
| `malformed-doc-tag` | TYPL-409 | warn | `@see` or `@since` with a missing or malformed value |
| `doc-comment-style` | TYPL-410 | allow | doc comment written as `/** */` |
| `missing-timing` | RIDL-100 | warn | `signal` or `event` without a timing annotation |
| `degenerate-timing-range` | RIDL-108 | warn | degenerate timing range `@[X..X]` |
| `missing-response-bound` | RIDL-112 | warn | `command` or `query` with no declared response bound |
| `error-typed-parameter` | RIDL-304 | warn | `error`-typed or result-union parameter on a `command` or `query` |
| `ensure-without-result` | RIDL-305 | warn | `ensure` clause that never references `result` |
| `contract-error-name-in-enum` | RIDL-307 | warn | contract-error category name declared in an `error` enum |
| `named-result-union-in-query` | RIDL-308 | warn | named result union in query return position |
| `query-named-like-mutation` | RIDL-404 | warn | query named like a mutation |
| `shared-error-type` | RIDL-405 | info | one `error` type shared across unrelated failure domains |
| `redeclared-envelope-metadata` | RIDL-406 | info | payload struct re-declares envelope metadata |
| `ordinal-changed` | RIDL-407 | warn | interaction, struct field, or union arm ordinal changed against the published baseline |
| `low-cohesion-interface` | RIDL-414 | info | interface members form disconnected type-sharing groups |
| `package-fan-out` | RIDL-415 | info | package depends on too many workspace packages |
| `redundant-provider-set` | RSDL-409 | warn | a `requires` resolves to a redundant provider set |
| `unclaimed-backend-key` | RSDL-804 | warn | a backend key whose namespace no configured backend claims |
| `unknown-manifest-key` | MANI-005 | warn | unknown manifest key |
| `unknown-lint` | MANI-010 | warn | `[lints]` entry names no lint, or its value is not a level |

Setting `ordinal-changed` to `allow` makes `ridl check --baseline` report
nothing for ordinal changes, so the baseline desk check no longer warns about
them.
