# ADR-0026 — Doc comments: every carrier, links that resolve, four tags, the doc lints, and root discovery from a workspace member

## Status

Accepted — 2026-10-04. Scope: where a doc comment may stand and where one is
required, what a doc holds (links and tags), how a link resolves and what the IR
stores of it, the doc lints, the rule extraction that the editor shows, and how
the toolchain finds the root to load when it is entered at a workspace member.
It binds the typl reference §14, every carrier the checker reads, the IR's doc
fields, every later doc lint, and every entry point that loads a workspace.

Written from spec 2a of the devex and agent tracks brief, "documentation in the
source", and issue #529. Sebastien took decisions D-1 to D-6 of the design in
the brainstorming session of 2026-10-04 and agreed the design section by
section; he narrowed D-4 (decision 4 below) when planning found that most
declarations have no attribute block. The design numbers its decisions D-1 to
D-6; in this record decision 3 is design D-3, decision 4 is D-4, decision 5 is
D-5, decision 9 is D-6 and decision 10 is D-1. Decision 7 records D-2 with the
lint table of the reviewed design. The plan's controller took the rulings
recorded in decisions 4 (`@deprecated` on every carrier), 5 (remote packages)
and 10 (a member build that fails on another member's error) while it
implemented the design.

It amends three records in place, in the same change:

- [ADR-0002](ADR-0002-module-system.md) §4 gains root discovery (decision 10).
- [ADR-0024](ADR-0024-lint-registry-and-levels.md) decisions 1, 8, 12 and 15
  gain the lint that is `allow` by default (decision 8), and its decision 9 is
  replaced (decision 10).
- [ADR-0025](ADR-0025-workspace-aware-mcp-tools.md) decision 2 states the new
  root discovery for the MCP path mode (decision 10).

The language rules are in the
[typl reference](../specification/typl-language-reference.md) §14 and §16.5, the
user-facing description is
[the documenting chapter of the book](../book/documenting.md), and the lint
names are on [the lints page](../book/lints.md).

## Context

RIDL accepted `///` and `/** */` doc comments, but most of them were lost on the
way to a reader. The syntax tree held every doc comment as trivia, and the
checker read the docs of package-level declarations, interfaces, interactions
and services only. Fields, enum values, enumset bits and union arms had a `doc`
field in the IR that the checker always left empty. A parameter and every rsdl
declaration had no doc field at all. A doc comment in a position nothing read
was dropped with no diagnostic, a `[TypeName]` link was not resolved, `@see` was
not checked, and nothing reported a missing doc. The language server showed a
doc on hover for some declarations and none for a field, and completion items
carried no documentation.

Opened on a file inside a workspace member, the language server, `ridl check`
and the MCP path mode loaded the member alone (#529). An import of a sibling
member did not resolve, and the root's `[lints]` did not apply, which ADR-0024
decision 9 recorded as a limitation.

## Decision

1. **A doc comment documents the next named declaration or member, and every
   named declaration and member is a carrier.** The checker reads the doc of
   each carrier, and stores it in the IR where the IR has a field for it:

   | Carrier                                                        | Before this record                      | Now                                                          |
   | -------------------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------ |
   | `type`, `const`, `struct`, `enum`, `enumset`, `union`          | read                                    | unchanged                                                    |
   | `interface`, `service`, each interaction                       | read                                    | unchanged                                                    |
   | `reserved` entry                                               | in the tree; no IR field                | a carrier with no IR field; its links are checked (TYPL-401) |
   | struct field, enum value, enumset bit, union arm               | in the tree; the IR field is left empty | read                                                         |
   | parameter of a `command` or `query`                            | no carrier                              | read into `Param.doc`                                        |
   | `system`, `component`, `distribution`, `deployment`, `machine` | in the tree; no IR field                | read into the system IR                                      |
   | rsdl body line (`offers`, `requires`, a bare member reference) | in the tree; no IR field                | read, see below                                              |

   The docs of the `system` and `distribution` member lines and of the `offers`
   and `requires` lines are stored in the system IR. A placement line in a
   `machine` body is a carrier and the editor shows its doc, but the system IR's
   `Placement` has no doc field. A `reserved` entry is the same: its doc is read
   and its links are checked, and the IR's `Reserved` has no doc field.

   These positions are not carriers: before `package`, before an `import`,
   before a return type or an attribute block, an arm of an inline `T | E`
   return, and the end of a file or of a body. Each either has no name to
   document or is documented by the item around it. A doc comment there draws
   TYPL-407 (decision 7).

2. **A doc is CommonMark, and three bracket forms are link candidates:**
   `[Name]` (and `[pkg.Name]`), the code-span form ``[`Name`]``, and the
   labelled form `[text][Name]`. The bracket content is a candidate only when it
   is a qualified identifier with at most one member suffix, so `[0..250]` and
   `[see below]` are prose. A candidate inside a code span or a fenced code
   block is not a link, and a `[Name]` that has a CommonMark link reference
   definition in the same doc is an ordinary Markdown link. Examples are written
   under a `# Examples` heading with a fenced block; this is a convention, not a
   tag, and the compiler does not read it. The scanner reads the doc with
   `pulldown-cmark`, keeps a map from each byte of the doc to its source offset
   so that a link and a tag problem carry an exact source span, and handles a
   `/** */` comment with `*` line decoration and CRLF line endings.

3. **A parameter doc is allowed and never required** (design D-3). A parameter's
   type is always a named typl type (general form R6), so the type's doc and
   rules document it. The editor shows the type's doc when a parameter has none.
   The rsdl body lines also take a doc and never require one.

4. **The tags are `@see`, `@since`, `@deprecated` and `@labels`** (design D-4).
   A tag is `@word` at the start of a line of the doc, after the comment markers
   and leading whitespace; an `@` anywhere else is prose. Each tag may appear
   more than once, and every value is kept.

   | Tag           | Value                                           | Checked                               |
   | ------------- | ----------------------------------------------- | ------------------------------------- |
   | `@see`        | one qualified name, with an optional member     | resolved like a link (decision 5)     |
   | `@since`      | a version: `MAJOR.MINOR` or `MAJOR.MINOR.PATCH` | its form only; compared with nothing  |
   | `@deprecated` | a quoted reason string                          | a missing reason is TYPL-405          |
   | `@labels`     | comma-separated `SCREAMING_SNAKE` labels        | passed through unchecked (typl §14.3) |

   Any other `@word` at the start of a line draws TYPL-408. A `@see` or `@since`
   with a missing or malformed value draws TYPL-409. `@see` and `@since` pass
   the deletion test of the general form §4.1: no tool output depends on them
   except rendered documentation. `@deprecated` and `@labels` keep their
   behaviour: the IR carries them, the editor shows them, the catalog hash
   clears them, and `ridl diff` classifies a change to them as `DocOnly`. A
   carrier whose IR has a `deprecated` field takes it from the tag — a
   declaration, a struct field, an interface, an interaction and a service — and
   TYPL-405 fires on a bare `@deprecated` on each of them. Struct fields also
   take their labels from `@labels`.

   **The general form §4.7 promotion of `deprecated` and `labels` to attribute
   keys is deferred to spec 2b.** An attribute block exists on the interactions
   `signal`, `event`, `command` and `query` (RIDL-106 rejects one on `fixed`)
   and on the rsdl declarations and body lines. `type`, `const`, `struct`,
   `enum`, `enumset`, `union`, `interface`, `service`, fields and enum values
   have none, so removing the doc tags would leave them, and `fixed`, with no
   way to be deprecated. No output of this record changes when the tags change.
   The first consumer that would need the promotion is generated deprecation
   metadata, and spec 2b decides between adding the attribute block to the
   grammar and reading the fact from the doc tag.

5. **A link resolves in the scope of the file that holds it, by the rules of a
   type reference, and the IR stores each resolved target** (design D-5).
   `ridl_sem::resolve_doc_link` is the one resolver; the checker and the
   language server both call it.
   - A bare `Name` is looked up in the file's package in the order a type name
     is: local, then imported (an alias leads to the declaration it names), then
     `ridl.std`.
   - A qualified `pkg.Name` names any package the current package can depend on
     under ADR-0002 §5, whether or not the file imports it, so a doc link never
     forces an import the code does not use. A remote package of `[imports]` is
     not materialized by the loader, so a link into one does not resolve and
     draws TYPL-401; that reach cannot be used until remote packages are loaded.
   - One more segment names a member: a field, enum value, enumset bit, union
     arm or interaction of the named declaration.
   - A target that is `internal` in another package does not resolve, because
     rendered documentation of that package could not follow it. An `internal`
     target in the same package resolves.

   A link or `@see` that does not resolve draws TYPL-401 and is not stored. Each
   one that resolves is stored as `DocLink { text, offset, len, target }`:
   `offset` and `len` are byte offsets into `doc` (zero for a `see` entry), and
   `target` is the canonical qualified name with an optional `.member`.

6. **The IR changes add fields only** (ADR-0014). `Param` gains `doc`. Every IR
   v2 message that has `doc` gains `repeated DocLink links`,
   `repeated DocLink
   see` and `repeated string since`. In the system IR,
   `System`, `MemberLine`, `Component`, `Offer`, `Require`, `Distribution`,
   `Deployment` and `Machine` gain `doc`, the links, `see` and `since`; on
   `Deployment` the links field is `doc_links`, because `links` already names
   the link set there. `catalog_hash` clears `links`, `see`, `since` and the
   parameter doc, as it already cleared `doc`, `labels` and `deprecated`; a test
   deletes every doc comment of a fixture workspace and checks that the hash is
   unchanged. `ridl diff` classifies a change to any of these fields as
   `DocOnly`, and `diff_systems` ignores the system doc fields. The codegen
   model of the plugin protocol (`codegen/v1/model.proto`) gains none of them:
   rendering links, `@see` and `@since` in generated code is spec 2b.

7. **The doc lints are `TYPL-` codes, all Warning** (design D-2 and the reviewed
   lint table). Doc comments are typl §14, shared by every language of the
   family, so the codes apply in `.typl`, `.ridl` and `.rsdl` files.

   | Code     | Lint name                   | Default | Raised when                                                             |
   | -------- | --------------------------- | ------- | ----------------------------------------------------------------------- |
   | TYPL-401 | `broken-doc-link`           | `warn`  | a link or `@see` target does not resolve (decision 5)                   |
   | TYPL-404 | `detached-doc-comment`      | `warn`  | a blank line separates a doc comment from its carrier, on every carrier |
   | TYPL-405 | `deprecated-without-reason` | `warn`  | `@deprecated` with no reason string                                     |
   | TYPL-406 | `missing-docs`              | `warn`  | a covered item has no doc                                               |
   | TYPL-407 | `misplaced-doc-comment`     | `warn`  | a doc comment in a position that is not a carrier                       |
   | TYPL-408 | `unknown-doc-tag`           | `warn`  | a tag other than the four of decision 4                                 |
   | TYPL-409 | `malformed-doc-tag`         | `warn`  | `@see` or `@since` with a missing or malformed value                    |
   | TYPL-410 | `doc-comment-style`         | `allow` | a doc comment written as `/** */`                                       |

   `missing-docs` covers a `type`, `const`, `struct`, `enum`, `enumset`,
   `union`, `interface` or `service` that is not `internal`; each field, enum
   value, enumset bit, union arm and interaction of one; and every rsdl
   `system`, `component`, `distribution`, `deployment` and `machine`. It never
   covers a parameter, a `reserved` entry, an rsdl body line, a member of an
   `internal` declaration, or the package. A doc made only of tags is missing.
   There is one diagnostic per item, at its name, with a fix-it that inserts a
   `///` line above the item when the item starts its line. When the entry is
   inside a workspace member, `ridl check`, `ridl build`, `ridl lock` and the
   MCP path mode report only the diagnostics of files under the member (ADR-0024
   decision 9, as decision 10 below replaces it), so a sibling member's
   undocumented items are not reported there; the language server reports every
   loaded file. A remote package of `[imports]` is never checked.

   `doc-comment-style` checks one direction: a project that sets it to `warn` or
   `deny` requires the house style `///`. Its fix-it rewrites the comment as
   `///` lines. Requiring `/** */` would need a lint option, which `[lints]`
   does not have.

8. **A catalogue row may declare `allow` as its default level.** This amends
   ADR-0024 decisions 1, 8, 12 and 15. Only a Warning or Info row may declare
   it, which a catalogue guard test checks. `ridl_core::lint::default_level`
   returns `Some(LintLevel::Allow)` for such a row and the severity-derived
   level for any other Warning or Info row; it still returns `None` for an Error
   row. A project that sets the lint to `warn` or `info` gets that severity, and
   `deny` gives Error. In SARIF, the rule's `defaultConfiguration.level` is
   `none`. The paths that apply no lint levels (`ridl baseline`, `ridl lock`,
   `ridl test`, and the compile error of `ridl diff`) drop the diagnostics of
   such a row, so the `allow` default holds on every path and a command that
   applies no levels never prints a lint that no project turned on.
   `docs/book/lints.md` shows the default level of each lint. TYPL-410 is the
   first row that uses it.

9. **One public function extracts the rules of an item, and the editor renders
   them** (design D-6). `ridl_ir::rules(pkg, deps, item)` reads IR v2 and
   returns a list of `Rule`: `Range`, `Step`, `Unit`, `Length`, `Pattern`,
   `CollectionBound`, `Timing` (the rate or staleness range of a `signal` or
   `event`), `ResponseBound` (of a `command` or `query`), `Errors` (the error
   type of a fallible return) and `Contract` (a `require` or `ensure` clause as
   source text). For a field or parameter whose type is a named declaration, the
   function returns that declaration's rules, looked up in `pkg` or in `deps`.
   It adds nothing to the IR. There is no `Scale` variant, because the IR holds
   no scale. The shape of `Rule` is provisional: it is an input to spec 2b,
   which decides whether the IR stores the list or each backend calls the
   function, and may change the shape. Until then, hover is its only consumer.

10. **Root discovery walks from a workspace member to its workspace, for every
    entry point** (design D-1, #529). `ridl_core::find_root` implements the rule
    of ADR-0002 §4: when the nearest `ridl.toml` at or above the entry is a
    `[package]` manifest, the walk continues upward and stops at the first
    `[workspace]` manifest, which is the root when its `members` names the
    package directory; a manifest that cannot be read or parsed, a directory
    that holds `.git`, and the filesystem root also stop it. `ridl check`,
    `ridl build`, the other commands, the MCP path mode and the language server
    all call it. A member entry then:
    - applies the root's `[lints]`, `[defaults].timing` and `[imports]`, and
      resolves imports of sibling members;
    - reports only the diagnostics of files under the member, in `ridl check`,
      `ridl build`, `ridl lock` and the MCP path mode, while the language server
      publishes the diagnostics of every loaded file;
    - fails a `ridl build` with one explanatory error, and writes nothing, when
      another member has an error, because a build must not succeed over a
      workspace that does not compile;
    - names the crate of `ridl build <member> --emit rust` `ridl_generated`, as
      for the workspace root, and uses the root's `ridl.lock` and
      `.ridl/baseline/`.

    In the language server, a root manifest that draws a manifest error and
    yields no package is one load error, shown once and naming the root
    `ridl.toml`, whichever of `initialize` and the first `didOpen` finds it.
    This replaces ADR-0024 decision 9 ("entering at a workspace member loads the
    member alone") and changes ADR-0025 decision 2, whose note for a member path
    is removed.

### Checks against the language-surface records

- [ADR-0011](ADR-0011-provisioned-constant-keyword.md) (the provisioned constant
  keyword): no interaction. `const` keeps its carrier.
- [ADR-0012](ADR-0012-interaction-boundary-model.md): no attribute key is added,
  so the fail-closed rule of its decision 9 is not engaged. `DocOnly` keeps its
  meaning ("doc comment, labels, or deprecation metadata") and also covers the
  new doc fields of decision 6.
- [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md): no interaction.
  RIDL-106 is unchanged; a doc comment on `fixed` is a carrier as on any other
  interaction.

## Alternatives considered

From the design's alternatives. The numbers are the decisions that reject them.

- **Fix #529 in the language server only.** Less risk to the command line, but
  the editor and `ridl check` would report different levels for one file, which
  ADR-0024 decision 3 exists to prevent. **Leaving #529 out** keeps cross-member
  links and the root's levels unavailable in the editor. Rejected for
  decision 10.
- **rsdl not covered by `missing-docs`.** Avoids the system IR changes, but
  leaves the top layer of the family undocumented. **Requiring a package doc**
  needs a rule for which of a package's files holds it, and a diagnostic for
  two. Rejected for decision 7.
- **Requiring parameter docs.** Signature help and generated code would always
  have a parameter doc, but every parameter type is a named, documented type, so
  the doc would mostly repeat it. **No parameter carrier** (a `# Parameters`
  section) leaves hover on a parameter with nothing of its own. Rejected for
  decision 3.
- **Carrying out the general form §4.7 promotion now.** It needs an attribute
  block on every declaration and member — a grammar, parser, formatter and
  lowering change — and nothing here consumes the result. **Promoting only where
  a block exists** (the interactions other than `fixed`, and rsdl) gives two
  spellings of one fact. **An `@example` tag** duplicates a Markdown heading and
  a fenced block. Rejected for decision 4.
- **Qualified links only to imported packages.** A simpler resolver, but a doc
  link would force an import the code does not use. **Raw link text in the IR**,
  resolved by each backend, repeats the resolver in every backend, and an
  out-of-repository plugin cannot do it. Rejected for decision 5.
- **Hover formats the rules itself.** Less work now, but spec 2b would write a
  second formatter that can disagree with the editor. **Defining the IR's rule
  data now** takes a decision that spec 2b and the catalog descriptor work (Epic
  16) should take together while Epic 16 still changes the descriptor IR.
  Rejected for decision 9.
- **Signature help on declarations.** Signature help serves call sites, and the
  grammar has no call expression (`call_expr` is expr-core V2, roadmap E5.1).
  The editor shows parameter docs on hover and in completion.
- **A `doc-comment-style` option to require `/** */`.** `[lints]` has no lint
  options, and adding them is a registry change for one lint. Decision 7.

## Consequences

- Member docs now reach the backends, because the checker fills the IR fields
  they already read. The backend code does not change, but its output does: the
  Rust and TypeScript backends render the doc of a struct field, an enum value,
  an enumset bit, a union arm and an interaction, which was empty before, and a
  tag line no longer appears in a rendered doc, because the scanner removes
  every tag line from `doc` — `@since` lines included, which it kept before. A
  struct field or interaction with a `@deprecated` tag now carries the
  deprecation in the IR, so a backend that emits deprecation metadata for
  members emits it for them.
- The catalog hash does not change when a doc, a link, a tag or a parameter doc
  changes, and `ridl diff` reports such a change as `DocOnly`.
- A project that adopts this toolchain sees `missing-docs` warnings for every
  undocumented covered item. It can set the lint to `allow` in `[lints]`.
- A member entry point changes behaviour: the root's `[lints]`,
  `[defaults].timing` and `[imports]` apply, a member build fails on another
  member's error, the generated crate of a member build is named
  `ridl_generated`, and the lockfile is the root's. The CHANGELOG entry states
  this.
- A link into a remote `[imports]` package draws TYPL-401 until the loader
  materializes remote packages.
- `ridl-sem` depends on `pulldown-cmark` (without default features), so the
  scanner reads CommonMark as mdBook does; `just wasm-check` still passes.
- The language server shows the signature, the doc, a **Contract** list from
  decision 9, and `@since`, labels and deprecation on hover for declarations,
  members, parameters, rsdl declarations and body lines, and for members used in
  contract clauses. Completion items carry the doc, except a member item after
  `.` in a doc link. In a doc comment, `[` and `.` complete link targets and `@`
  at the start of a line completes the four tags. Go-to-definition,
  find-references and rename work on doc links and `@see` targets in `.typl`,
  `.ridl` and `.rsdl` files. There is no signature help.

## References

- [ADR-0002](ADR-0002-module-system.md) §4 and §5 — the manifest, root discovery
  and package resolution; §4 amended.
- [ADR-0024](ADR-0024-lint-registry-and-levels.md) — the lint registry and
  levels; decisions 1, 8, 12 and 15 amended, decision 9 replaced.
- [ADR-0025](ADR-0025-workspace-aware-mcp-tools.md) decision 2 — the MCP path
  mode; amended.
- [ADR-0014](ADR-0014-ir-encodings.md) — the IR's encodings and its
  compatible-change rule.
- [The typl reference](../specification/typl-language-reference.md) §14 (doc
  comments) and §16.5 (the documentation codes).
- [The documenting chapter of the book](../book/documenting.md) and
  [the lints page](../book/lints.md).
