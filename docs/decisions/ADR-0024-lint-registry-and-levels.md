# ADR-0024 — The lint registry and levels: names, the `[lints]` table, where levels apply, and SARIF

## Status

Accepted — 2026-10-04. Scope: how a warning or info diagnostic becomes a
configurable lint — which diagnostics are lints, how a project sets a level,
which commands apply the levels, how a level resolves for a file, and how
`ridl check --format sarif` projects a diagnostic. It binds every later lint
(the planned documentation lints and the planned design lints), every entry
point that reports diagnostics, and the lint names, which a `ridl.toml` refers
to.

Written from spec 0 of the devex and agent tracks brief, implemented in
driftsys/ridl#678. Sebastien agreed decisions 1 to 7 in the brainstorming
session of 2026-10-03. The maintainer's delegate took decisions 8 and 9 in the
pass-1 review of the design, PR #671. Decisions 11 and 12 and the base rule of
decision 10 come from the reviewed design (§5.3, §7.3 and §5.2/§6.1). The stage
driver added the root-scope rule for an unowned file in decision 10, the choice
in decision 12 to map the default level from the catalogue severity rather than
through `default_level`, and decisions 13 to 16 while it implemented the design,
and recorded them in the body of #678. The original design and plan are
[`docs/archive/2026-10-03-lint-foundation-design.md`](../archive/2026-10-03-lint-foundation-design.md)
and
[`docs/archive/2026-10-03-lint-foundation-plan.md`](../archive/2026-10-03-lint-foundation-plan.md);
the design's §10 holds the alternatives, restated below.

It amends two records in place, in the same change:
[ADR-0002](ADR-0002-module-system.md) §4 (the `[lints]` table and its resolution
order) and [ADR-0010](ADR-0010-cli-conventions.md) decision 1 (a lint at `deny`
exits 1, and the other subcommands apply no level). Those records state the
contract their readers need; this record holds the reasoning.

The user-facing description is [the lints page of the book](../book/lints.md),
which also holds the table of lint names. The output formats are in
[the CLI reference](../book/cli-reference.md).

## Context

The compiler already emitted warnings and infos, and none of them could be
configured. `Severity` is `Error | Warning | Info`; each emit site chose its
severity, so the catalogue's severity was a default only, and some sites
hard-coded their own. `ridl check` exited 1 only when an Error was present, with
no flag to change that. `ridl check --format` accepted `text` and `json`, so a
CI system that reads SARIF could not show findings.

The planned documentation lints (`missing-docs`, a broken `[Type]` link) and the
planned design lints need a level per lint, set per project, with the same
result in the command line, the language server and the MCP server.

The design numbers its decisions D-1 to D-9. In this record, **design D-n is
decision n** for n from 1 to 9, so a citation of "D-8" is a citation of decision
8.

## Decision

1. **Every Warning and Info catalogue code is a lint** (design D-1). Each gets a
   stable kebab-case name, carried by its row in the diagnostic catalogue, and
   its catalogue severity is its default level (Warning is `warn`, Info is
   `info`). An Error code is never a lint and can never be configured. There is
   one diagnostic channel and one catalogue; a lint is a catalogue row that has
   a name. A catalogue guard test keeps the names well formed: every Warning or
   Info row has one, no Error row has one, every name matches
   `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`, and no two rows share one.

   One consequence the maintainer accepted: RIDL-407 (`ordinal-changed`), the
   warning of the `ridl check --baseline` gate, is a lint, so a project can set
   it to `allow` and make that gate silent for ordinal changes. The book states
   this next to the lint.

2. **A level is set only in the `[lints]` table of a `ridl.toml`** (design D-2).
   There is no source attribute such as `@allow(...)`. It stays a possible
   follow-up.

3. **The command line does not override levels** (design D-3). There is no
   `--deny-warnings` and no `-A`/`-W`/`-D` flag. A CI that wants stricter checks
   commits the levels to the manifest, so the editor, the command line and an
   agent agree for the same entry point.

4. **The levels are `allow`, `info`, `warn` and `deny`** (design D-4). `allow`
   removes the diagnostic. `deny` emits it with Error severity, so ADR-0010's
   rule ("a diagnostic error over the checked source" exits 1) applies with no
   new exit code. There is no `forbid`: without a source attribute there is
   nothing for it to forbid.

5. **The SARIF output carries no `helpUri` yet** (design D-5). The long-form
   error index (roadmap E4.2) is not written, and the MCP tool `ridl_explain`
   serves the catalogue offline. A `helpUri` is added when an index page exists.

6. **The registry and the step that applies levels live in `ridl-core`** (design
   D-6, approach A). The catalogue rows carry the names (`ridl_core::lint` looks
   them up), the manifest parser reads `[lints]`, and one function,
   `apply_lint_levels`, applies the levels over a diagnostic list. It is
   idempotent. The shared compile in `ridlc` does not apply levels; it carries
   the loaded `LintScopes` out on its outputs, and the entry points that report
   diagnostics apply them:
   - `ridlc::run_check` always; `ridlc::run_build_with` before its success gate
     and before it writes an artifact, when its caller passes `ApplyLints::Yes`
     (`ridl build` and `ridlc build` do; `ridl baseline` does not);
   - `ridl check --baseline`, once more over the whole run, because the CLI
     itself raises RIDL-407 after `ridlc` returns;
   - `ridlc::front_end`, with empty scopes, for `check_source` and `compile`;
   - the MCP tool `ridl_check` in path mode, with the scopes of
     `WorkspaceOutput`;
   - the language server, which applies them to the loader diagnostics in
     `load()` and to its own in `analyze`, before converting either.

   `ridlc::front_end`, which `check_source` and `compile` call, applies the
   levels with empty scopes, so only the registry defaults apply to a single
   source with no manifest.

7. **The lint names are a stable contract once released** (design D-7). A
   `ridl.toml` refers to a name, so a released name is never renamed or reused.
   A lint whose code is retired keeps its name reserved, as `RETIRED_RIDL_CODES`
   reserves codes. There is no alias mechanism. The table of names is not copied
   here: the catalogue (`diag_codes!` in `crates/ridl-core/src/diag.rs`) is its
   source, and [the lints page](../book/lints.md#the-lints) lists it, checked
   against the catalogue by `crates/ridl/tests/book_lints.rs`. A new lint adds
   its name to the catalogue row and a row to that page in the same change. It
   also updates the expected list in the `ridl-core` diagnostic test that
   compares the catalogue with the lint names.

8. **Levels apply only where diagnostics are reported to a person or an agent**
   (design D-8). These are `ridl check` (including the `--baseline` desk check),
   `ridl build`, `ridlc check`, `ridlc build`, the language server, and the MCP
   tool `ridl_check`. Every other command (`ridl diff`, `ridl test`,
   `ridl baseline`, `ridl lock`, the MCP tool `ridl_diff`, the MCP lookup tools)
   compiles with the severities the emit sites chose, so a lint at `deny` never
   makes one of them fail or exit 2. A `[lints]` table is a reporting setting;
   it does not change whether a workspace compiles. ADR-0010 decision 1 states
   the exit-code consequence.

9. **Entering at a workspace member loads the member alone** (design D-9).
   `ridl check <member>`, `ridl check` on a file inside a member, the MCP path
   mode on a member, and an editor opened on a member load the member as a
   standalone package, so the workspace root's `[lints]` does not apply. This is
   how `[defaults].timing` and `[imports]` behave, and it is part of the
   language server gap #529, which stays open. This decision does not change the
   loader's root discovery.

10. **A diagnostic takes the levels of the directory that owns its primary span,
    and the root's table covers the whole root tree** (design §5.2 and §6.1; the
    root-tree rule was taken at implementation). The loader builds a map from
    directory to effective levels: one scope for the workspace root, one per
    member, one for a standalone package, none in single-file mode. The lookup
    takes the longest directory, compared component by component, that is a
    prefix of the path of the primary span's file. The map is keyed by directory
    and not by file id because a file id does not always survive: the CLI's
    RIDL-407 spans are interned after `ridlc` returns, and a file created in the
    editor after the workspace loaded has no entry. A file under the workspace
    root that no member owns, and the root `ridl.toml` itself, get the root's
    `[lints]`. A file outside the entry point's directory tree, or with no path,
    gets the registry defaults, so a dependency can never fail a project's check
    and a project's table never applies to code outside its directory.

11. **A mistake in `[lints]` never stops a check** (design §5.3). A key that is
    not a registered lint name (including a diagnostic code and the name of an
    Error code), a value that is not a string, a string that is not a level, and
    a `lints` key that is not a table each raise MANI-010 (`unknown-lint`,
    Warning) on the entry, and the entry is ignored. MANI-010 is itself a lint,
    so the table can set its level.

12. **The SARIF log is a small hand-written projection of SARIF 2.1.0** (design
    §7.3). It lives in `crates/ridl-core/src/diag/sarif.rs`, so the MCP server
    can reuse it, and uses no SARIF crate. `tool.driver.rules` lists every
    catalogue row, errors included. A rule's `defaultConfiguration.level` is
    mapped directly from the catalogue severity (Error to `error`, Warning to
    `warning`, Info to `note`), and a result's `level` from its effective
    severity by the same mapping. `columnKind` is `unicodeCodePoints`, because
    RIDL columns count characters. The log carries no fix-its, and an `allow`ed
    diagnostic does not appear. A diagnostic with no path in the source map has
    no `locations`.

13. **SARIF artifact URIs have one base, the working directory.** A file under
    the working directory is a URI relative to it, with `/` separators and every
    segment percent-encoded, and carries `uriBaseId` `%SRCROOT%`; the run's
    `originalUriBaseIds` maps `%SRCROOT%` to the working directory as a
    `file://` URI that ends with `/`. A relative source path is joined onto the
    working directory, and `.` and `..` are resolved lexically, never through
    the filesystem. A file outside the working directory is an absolute
    `file://` URI with no `uriBaseId`; so is a file reached through a symbolic
    link that the working directory's physical path does not spell. A leading
    `..` of a relative path stays when there is no working directory to join
    onto. When the working directory cannot be read, the run has no
    `originalUriBaseIds`. A code-scanning upload run from the checkout root
    therefore resolves every URI.

14. **`ridl check --baseline` indexes declarations from the manifest root.** The
    desk check builds a declaration index so a RIDL-407 diagnostic carries a
    file. When the entry is a file or a subdirectory, the index covers the
    manifest root at or above it, the root `ridl_core::load_workspace` compiles
    from; a file with no manifest above it is indexed alone. Otherwise a change
    in a file above or beside the entry would carry a detached span, which no
    `[lints]` scope reaches, and its level would fall back to the default.

15. **`default_level` returns `Option<LintLevel>`, `None` for an Error row.** An
    Error row is not a lint and has no level; the type states that, and no
    caller can read a level off an Error code by mistake.

16. **ADR-0010's sentence names the two servers.** In the amended decision 1,
    "the other subcommands do not apply lint levels" excludes `ridl lsp` and
    `ridl mcp`: they apply levels to the diagnostics they report, and no exit
    code of theirs depends on a level.

## Alternatives considered

From the design's §10. The numbers are the decisions that reject them.

- **Only new lints in the registry; existing warnings stay fixed.** Smaller
  today, but two kinds of warning would behave differently, and the book would
  have to explain it. Rejected for decision 1.
- **Every warning is a lint except a few hazards** (MANI-005, the lockfile write
  failure, RIDL-407). Rejected by the maintainer: one rule is easier to state.
  The uncoded lockfile write warning stays fixed anyway, because it has no code.
- **A source attribute such as `@allow(missing-docs)`.** Gives a local exception
  without turning a lint off for the whole package, but changes the language
  surface (an ADR, checked against ADR-0011, ADR-0012 and ADR-0015). Deferred
  (decision 2) until real false positives from the planned lints show the need.
- **`--deny-warnings`, or cargo-style `-A`/`-W`/`-D` flags.** Convenient for CI,
  but the editor and CI could then disagree. Rejected for decision 3.
- **A `helpUri` into the book, or a long-form help text per code now.** The
  first needs an anchor per code kept in sync, the second about 150 help texts.
  Deferred to the error index (decision 5).
- **Pass the levels into `ridl-sem` so each emit site checks its level (approach
  B).** Skips the work for allowed lints but changes every emit site and pass
  signature, for a saving that does not matter at current sizes. Rejected for
  decision 6.
- **Apply the levels in each front end separately (approach C).** Touches the
  fewest files, but three copies drift, and the brief asks for the same results
  everywhere. Rejected for decision 6.
- **A `forbid` level.** Has nothing to forbid without a source attribute
  (decision 4).
- **Apply the levels inside the shared compile, for every command.** Simpler,
  with one call in `check_loaded`, but a lint at `deny` would then make
  `ridl diff` and `ridl test` exit 2, block `ridl baseline` and `ridl lock`, and
  make MCP `ridl_diff` report that the workspace does not compile. Rejected for
  decision 8: a lint level is a reporting setting.
- **Walk up from a member to its workspace root to find the root `[lints]`.**
  Would give a member entry point the same levels as a root entry point, but it
  changes the loader's root discovery, and with it how `[defaults].timing` and
  `[imports]` behave at a member, which belongs with the work on #529. Rejected
  for this record (decision 9).
- **A SARIF crate** (`serde-sarif`). The subset used is a few structs; a
  dependency is not worth it (decision 12).

The design records no alternatives for decisions 10, 11 and 13 to 16, and the
stage driver recorded none for them at implementation.

## Consequences

- A lint added by a later spec adds a catalogue row with a name and a row on the
  lints page, and needs no change to the levels machinery. A code that an emit
  site hard-codes to a severity still follows the project's level where levels
  apply (decision 6), and keeps the hard-coded severity elsewhere (decision 8).
- `ridl build` stops on a lint at `deny` before it writes any artifact. The
  other commands of decision 8 are unaffected by a project's `[lints]`.
- Output formats: the JSON report gains an optional `lint` field (an addition,
  which the contract stability policy of ADR-0005 §7 allows), the text report
  adds a note naming the lint (it depends only on the code, so it also appears
  for commands that apply no level), and `ridl check --format sarif` is new. The
  MCP tool `ridl_explain` adds `lint` and `default_level` for a lint code.
- Known limitations, both of the language server: it reads the `[lints]` tables
  once, when it loads the workspace, so an edit to a table takes effect after a
  restart; and it does not canonicalise paths when it looks up a scope. The
  member entry point gap (decision 9) is #529.
- Diagnostic codes written in Markdown stay unguarded (#191) except the lint
  table of the book, which `book_lints.rs` compares with the catalogue.

## References

- [ADR-0002](ADR-0002-module-system.md) §4 — the manifest; amended with the
  `[lints]` table.
- [ADR-0010](ADR-0010-cli-conventions.md) decision 1 — exit codes; amended.
- [ADR-0005](ADR-0005-agent-enablement.md) §7 — the agent-legibility invariants,
  including the contract stability policy.
- [The lints page](../book/lints.md) and
  [the CLI reference](../book/cli-reference.md).
- [The toolchain architecture technote](../technotes/walking-skeleton-architecture.md)
  — where the `lint` module, `diag::sarif` and the call sites sit.
