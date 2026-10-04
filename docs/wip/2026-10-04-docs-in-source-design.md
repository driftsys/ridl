# Documentation in the source — design for spec 2a

Status: design spec for Spec 2a of
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md),
written 2026-10-04 against `main` at f3f12982. Sebastien took decisions D-1 to
D-6 in the brainstorming session of 2026-10-04 and agreed the design section by
section (§2). Nothing here is implemented. It is archived with its plan when the
work lands.

Satisfies: the brief's Spec 2a (the tag set, the doc lints, every doc comment in
the language server, hover with the extracted rules, `[Type]` links with
completion and go-to-definition, a doc-stub quick fix) and issue #529 (the
language server's member-entry gap). Builds on: ADR-0024 (the lint registry and
levels, merged as #678). Related: the general form §4.1 (the deletion test) and
§4.7 (promoted metadata), ADR-0002 §4 and §5 (manifest and package resolution),
ADR-0012 decision 9 (attribute keys and diff categories), ADR-0015 (RIDL-106),
issue #172 (attribute keys not consumable), spec 2b (documentation in generated
code, which consumes §6.4).

## 1. The problem

RIDL accepts `///` and `/** */` doc comments (typl §14), but most of them are
lost on the way to the reader:

- The syntax tree is lossless, so every doc comment is in it as trivia. The
  checker reads docs only for package-level declarations, interfaces,
  interactions and services (`docs::scan` in `crates/ridl-sem/src/docs.rs`).
  Fields, enum values, enumset bits and union arms have a `doc` field in the IR,
  but the checker always writes an empty string into it.
- A call parameter and every rsdl declaration have no doc carrier in the IR. The
  system IR (`system.proto`) has no `doc` field at all.
- A doc comment in a position nothing reads is dropped with no diagnostic.
- `[TypeName]` links are not resolved. TYPL-401 is in the typl reference but not
  in the catalogue.
- typl §14.2 still lists `@labels` and `@deprecated` as doc tags, and the
  checker still parses them, although the general form §4.7 moved both to
  attributes under the deletion test. `[deprecated = "…"]` is recognised and
  rejected as not consumable (FORM-107, issue #172).
- The language server shows a doc on hover for declarations, interfaces and
  interactions and their uses, but not for a field, and completion items carry
  no documentation.
- Opened on a file inside a workspace member, the language server loads the
  member alone (#529), so an import of a sibling member does not resolve and the
  root's `[lints]` does not apply (ADR-0024 decision 9).
- Nothing reports a missing doc.

## 2. Decisions taken in the brainstorming session

Decisions recorded in the brief (2026-10-03) and not reopened: source docs are
rustdoc-like Markdown plus a few tags for what the model cannot infer; `///` is
the house style and both forms stay accepted with the same meaning; `ridl fmt`
does not rewrite one form into the other; an optional `doc-comment-style` lint
is allowed by default; each interaction is documented on its member; the
interface doc states the interface's responsibility and does not repeat its
members; `missing-docs` is a warning by default and covers the interface and
each member.

Decisions taken on 2026-10-04:

- **D-1 — 2a fixes #529 for every entry point.** The loader walks up from a
  member's `[package]` manifest to the `[workspace]` manifest that lists it. The
  editor, `ridl check`, `ridl build` and the MCP path mode use the same rule
  (§8). This amends ADR-0002 §4 and replaces ADR-0024 decision 9.
- **D-2 — `missing-docs` covers every declaration that is not `internal`, its
  members, and the rsdl declarations.** The system IR gains doc fields in 2a. A
  package needs no doc (§5.2).
- **D-3 — A doc is allowed on every named declaration and member. A parameter
  doc is allowed and never required.** A parameter's type is always a named typl
  type (general form R6), so its type's doc and rules stand in for it. The rsdl
  member lines take a doc and never require one (§3.1, §5.2).
- **D-4 — The tags are `@see` and `@since`, and 2a carries out the §4.7
  promotion.** `deprecated` and `labels` become consumed attributes; the doc-tag
  forms are removed; `fixed` takes a block that holds only those two keys (§3.3,
  §3.4). Recorded in a new ADR-0025.
- **D-5 — A link resolves in the scope of the file that holds it, with the rules
  of a type reference.** A qualified link reaches any package the current
  package can depend on, without an import. One member level is allowed. The IR
  stores each resolved target (§6).
- **D-6 — 2a defines the rule extraction as one public function, and hover
  renders it.** 2b decides whether the IR stores the rules or calls the function
  (§6.4).

Agreed in the design review of the same session: the lint table and codes (§5),
the registry's allow-by-default row (§5.3), classifying a `labels` change as
compatible (§3.4), and removing signature help from 2a (§7.3).

## 3. The language surface

### 3.1 Carriers

A doc comment documents the next named declaration or member, as typl §14
states. 2a makes every one of these a carrier, from the syntax tree to the IR:

| Carrier                                                        | Today                                   | 2a                     |
| -------------------------------------------------------------- | --------------------------------------- | ---------------------- |
| `type`, `const`, `struct`, `enum`, `enumset`, `union`          | read                                    | unchanged              |
| `interface`, `service`, each interaction, `reserved` entry     | read                                    | unchanged              |
| struct field, enum value, enumset bit, union arm               | in the tree; the IR field is left empty | read                   |
| call parameter (`command`, `query`)                            | no carrier                              | new in the tree and IR |
| `system`, `component`, `distribution`, `deployment`, `machine` | in the tree; no IR field                | new in the system IR   |
| rsdl member line (`offers`, `requires`, bare reference)        | in the tree; no IR field                | new in the system IR   |

These positions are not carriers: `package`, `import`, a return type, an arm of
an inline `T | E` return, an attribute block, and the end of a file. A doc
comment there draws `misplaced-doc-comment` (§5.1). Each of them either has no
name to document or is documented by its enclosing item.

A `reserved` entry keeps its carrier but is never required to have a doc (§5.2).

### 3.2 Content

A doc is CommonMark, as typl §14.1 states. Three forms are link candidates, and
only when the bracket content is a qualified identifier with at most one member
suffix:

- `[Name]` and `[pkg.Name]`
- ``[`Name`]`` (the code-span form, as rustdoc accepts)
- `[text][Name]` (an explicit label)

A bracket whose content is not a qualified identifier (`[0..250]`,
`[see below]`) is prose, not a link. A candidate inside a code span or a fenced
code block is not a link. A `[Name]` that has a CommonMark link reference
definition in the same doc is an ordinary Markdown link, not a doc link.

Examples are written under a `# Examples` heading with a fenced block. This is a
convention, not a tag, and the compiler does not read it.

### 3.3 Tags

A tag is `@word` at the start of a line of the doc, after the comment markers
and leading whitespace. Two tags exist:

| Tag      | Value                                           | Checked                              |
| -------- | ----------------------------------------------- | ------------------------------------ |
| `@see`   | one qualified name, with an optional member     | resolved like a link (§6)            |
| `@since` | a version: `MAJOR.MINOR` or `MAJOR.MINOR.PATCH` | its form only; compared with nothing |

Each tag may appear more than once (`@see` usually does; a repeated `@since` is
allowed and every value is kept). Any other `@word` at the start of a line draws
`unknown-doc-tag`. For `@deprecated` and `@labels` the diagnostic says that the
fact moved to an attribute and offers a quick fix (§7.5). A known tag with a
missing or malformed value draws `malformed-doc-tag`. An `@` that is not at the
start of a line is prose.

Both tags pass the deletion test (general form §4.1): no tool output depends on
them except rendered documentation.

### 3.4 The promotion of `deprecated` and `labels`

The general form §4.3 lists `deprecated` and `labels` as attribute keys legal on
any declaration; §4.7 removes the doc-tag forms. 2a implements both:

- `deprecated = "reason"` and `labels = (A, B(C))` are consumed on every
  declaration and member that takes an attribute block. They fill the existing
  `labels` and `deprecated` fields of the IR `Decl`, `Field` and `Interface`,
  and the same fields are added where a carrier of §3.1 lacks them. The
  `GF_ATTRIBUTE_KEYS` list in `crates/ridl-sem/src/check.rs` becomes the key ×
  kind allow-list issue #172 calls for, for these two keys at least.
- `fixed` takes an attribute block that holds only `deprecated` and `labels`.
  RIDL-106 narrows from "an attribute block on `fixed`" to "an attribute block
  on `fixed` with a key other than `deprecated` or `labels`". This amends
  ADR-0015.
- `[deprecated]` written as a flag, with no reason string, draws TYPL-405, which
  keeps its code and its lint name `deprecated-without-reason`. Its text changes
  from "doc tag" to "attribute", as the general form §4.3 already proposed.
- Label validation by a profile (TYPL-402, TYPL-403) stays deferred, as ADR-0007
  decision 10 records.
- `ridl-diff` (ADR-0012 decision 9 requires a category for each consumed key):
  adding, removing or changing `deprecated` is a `Deprecation` change classified
  compatible; any change to `labels` is a `Labels` change classified compatible.
  An assurance profile may escalate a labels change; the core does not. Both
  leave `DocOnly`, which keeps only doc comments.
- The doc-tag forms `@deprecated` and `@labels` are removed from typl §14.2 and
  from `docs::scan`.

The human narrative of a deprecation stays in the doc comment; the
machine-readable fact is the attribute (general form §4.1).

## 4. Records changed

A new **ADR-0025** records D-3 to D-5, the carrier table, the tag set, the
promotion and its diff categories, and the RIDL-106 narrowing, with the
deletion-test argument and the ADR-0011, ADR-0012 and ADR-0015 checks:

- ADR-0011 (the provisioned constant keyword): no interaction. `const` keeps its
  carrier.
- ADR-0012: decision 9's fail-closed rule is met — both keys get a diff category
  (§3.4). Decision 9's description of `DocOnly` ("doc comment, labels, or
  deprecation metadata") is amended to "doc comment".
- ADR-0015: RIDL-106 narrows for the two metadata keys (§3.4).

Amended in place:

- typl reference §14 (carriers, content, tags, the attribute forms), §16
  (TYPL-401 implemented, TYPL-405 re-anchored, TYPL-406 to TYPL-410 added).
- ridl reference: the doc-comment notes and the RIDL-106 row. rsdl reference:
  the doc-comment notes for its declarations and lines.
- `docs/wip/family-general-form.md` §4.3 and §4.7: `deprecated` and `labels`
  marked as implemented; the RIDL-106 exception for `fixed`.
- ADR-0024: decisions 1 and 15 for the allow-by-default row (§5.3); decision 9
  replaced (§8).
- ADR-0002 §4: root discovery from a member (§8).
- The family overview: the decision ledger, ADR-0025 in the ADR list, and the
  sections its footer lists.

## 5. The doc lints

### 5.1 The table

Every doc lint is a `TYPL-` code: doc comments are typl §14, shared by every
language of the family, so the codes apply in `.typl`, `.ridl` and `.rsdl`
files. New codes start at TYPL-406.

| Code     | Lint name                   | Default | Raised when                                                                                                |
| -------- | --------------------------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| TYPL-401 | `broken-doc-link`           | `warn`  | a link or `@see` target does not resolve, or resolves to an `internal` declaration of another package (§6) |
| TYPL-404 | `detached-doc-comment`      | `warn`  | a blank line separates a doc comment from its carrier; extended from declarations to every carrier of §3.1 |
| TYPL-405 | `deprecated-without-reason` | `warn`  | `[deprecated]` with no reason string (§3.4)                                                                |
| TYPL-406 | `missing-docs`              | `warn`  | a covered item has no doc (§5.2)                                                                           |
| TYPL-407 | `misplaced-doc-comment`     | `warn`  | a doc comment in a position that is not a carrier (§3.1)                                                   |
| TYPL-408 | `unknown-doc-tag`           | `warn`  | a tag other than `@see` and `@since` (§3.3)                                                                |
| TYPL-409 | `malformed-doc-tag`         | `warn`  | `@see` or `@since` with a missing or malformed value (§3.3)                                                |
| TYPL-410 | `doc-comment-style`         | `allow` | a doc comment written as `/** */` (§5.4)                                                                   |

Each lint has a catalogue row with its name (ADR-0024 decision 1 and 7), a row
in `docs/book/lints.md`, and an entry in the `ridl-core` test that compares the
catalogue with the lint names.

### 5.2 What `missing-docs` covers

An item is covered when it is one of:

- a `type`, `const`, `struct`, `enum`, `enumset`, `union`, `interface` or
  `service` not marked `internal`;
- a field, enum value, enumset bit, union arm or interaction of a covered
  declaration (an error arm is an enum value or a union arm of an `error` type,
  so it is covered by the same rule);
- an rsdl `system`, `component`, `distribution`, `deployment` or `machine` (rsdl
  has no visibility; every rsdl declaration is covered).

Never covered: a parameter (D-3), a `reserved` entry, an rsdl member line, any
member of an `internal` declaration, and the package.

A doc made only of tags, with no prose, is missing. The diagnostic's primary
span is the item's name; there is one diagnostic per item. Code outside the
entry point's directory tree is never reported, because ADR-0024 decision 10
already gives it the registry defaults and the compile does not report its
diagnostics.

### 5.3 A lint that is allowed by default

ADR-0024 decision 1 makes a lint's default level its catalogue severity, so a
lint cannot be `allow` by default today. 2a adds an optional default to a
catalogue row:

- A row may declare its default level as `allow`. A guard test checks that only
  a Warning or Info row does.
- `ridl_core::lint::default_level` returns the declared default when there is
  one, and the severity-derived level otherwise. It still returns `None` for an
  Error row (ADR-0024 decision 15).
- A project that sets such a lint to `warn` or `info` gets that severity; `deny`
  gives Error, as for every lint.
- In SARIF, the rule's `defaultConfiguration.level` is `none` for a row whose
  default is `allow`.
- `docs/book/lints.md` shows the default level of each lint in its table.

This amends ADR-0024 decisions 1 and 15. TYPL-410 is the first row to use it.

### 5.4 `doc-comment-style`

The lint checks one direction: it reports a `/** */` doc comment, so a project
that sets it to `warn` or `deny` requires the house style `///`. Requiring
`/** */` instead would need a lint option, which `[lints]` does not have; that
is out of scope.

## 6. Links, the IR, and the rules

### 6.1 The resolver

The private `resolve_reference` in `crates/ridl-lsp/src/nav.rs` moves to
`ridl-sem` as a public function:

```rust
pub fn resolve_doc_link(
    db: &dyn Db, ws: &Workspace, pkg: PackageId, file: FileId,
    segments: &[&str],
) -> Result<LinkTarget, LinkError>
```

- A bare `Name` is looked up in `Resolution.symbols` of the file's package, in
  the existing order: local, then imported (an alias is followed to its
  declaration), then `ridl.std`.
- A qualified `pkg.Name` names any package the current package can depend on
  under ADR-0002 §5 (a workspace member, an entry of the package's `[imports]`,
  an entry of the workspace's `[imports]`), whether or not the file imports it.
- One more segment names a member: a field, enum value, enumset bit, union arm
  or interaction of the named declaration. `[Gear.PARK]`, `[Pose.x]`,
  `[CruiseControl.setLever]`.
- A target that is `internal` in a package other than the link's own is
  `LinkError::NotVisible`, because rendered documentation of the other package
  could not follow it.
- An ambiguity between a package path and a declaration with a member is
  resolved as a type reference resolves it today.

The checker calls the resolver for every link candidate and every `@see`, and
raises TYPL-401 for each error. The language server calls the same function for
go-to-definition, rename and completion (§7). The signature above is indicative;
the plan fixes it against the `ridl-sem` database API.

### 6.2 Scanning

`docs::scan` gains a CommonMark-aware pass. It uses `pulldown-cmark`, already a
workspace dependency (used by `crates/ridl`), to skip code spans and fenced
blocks, and it reads tags line by line. It returns:

```rust
pub struct DocInfo {
    pub text: String,          // the doc, markers stripped, tags removed
    pub links: Vec<DocLink>,   // candidates of §3.2, with byte ranges in `text`
    pub see: Vec<DocLink>,
    pub since: Vec<String>,
    pub problems: Vec<TagProblem>, // unknown or malformed tags, for §5.1
}
```

`labels` and `deprecated` leave `DocInfo`; they come from the attribute block
(§3.4).

### 6.3 The IR

All changes add fields to proto v2, so they are compatible changes under
ADR-0014.

- `Param` gains `doc`.
- Every message that has `doc` gains `repeated DocLink links`,
  `repeated DocLink
  see` and `repeated string since`.
  `DocLink { string text; uint32 offset;
  uint32 len; string target; }`:
  `offset` and `len` are byte offsets into `doc` (zero for a `see` entry), and
  `target` is the canonical qualified name with an optional `.member`. A link
  that did not resolve is not stored.
- In `system.proto`, `System`, `Component`, `Offer`, `Require`, `Distribution`,
  `Deployment`, `Machine` and `MemberLine` gain `doc`, `links`, `see` and
  `since`.
- `catalog_hash` already clears `doc`, `labels` and `deprecated`. It also clears
  `links`, `see`, `since` and the parameter doc. The system artifact's identity
  (ADR-0022) clears the new system fields in the same way. A test deletes every
  doc comment of a fixture workspace and checks that both hashes are unchanged:
  that is the deletion test, run directly.
- `ridl-diff` classifies a change to any of these fields as `DocOnly`.

The backends keep emitting what they emit today; member docs now reach them
because the checker fills the fields. Rendering links, `@see` and `@since` in
generated code is spec 2b.

### 6.4 The rules

A new module `ridl_ir::rules` provides one function:

```rust
pub fn rules(pkg: &Package, item: ItemRef<'_>) -> Vec<Rule>
```

`Rule` is an enum with one variant per fact the IR already holds:

- the value constraint: range, step, unit and scale;
- a collection bound and a string byte capacity;
- timing: the rate or staleness range of a `signal` or `event`, the response
  bound of a `command` or `query`;
- the error arms of an interaction, each with its error type's qualified name;
- each `require` and `ensure`, as source text.

For a field or parameter whose type is a named declaration, `rules` returns that
declaration's rules. The function reads only the IR and adds nothing to it. The
shape of `Rule` is provisional: spec 2b decides whether the IR stores the list
(for out-of-repo backends) or each backend calls the function, and may change
the shape when it does. Until then, hover is its only consumer.

## 7. The language server

### 7.1 Hover

Hover on a declaration, a member, or any use of either shows, in order:

1. the signature line, as today;
2. the rendered doc. A doc link becomes a Markdown link to the target's location
   (`file:` URI with a line fragment); a client that does not render links shows
   it as a code span;
3. a **Contract** list rendered from `ridl_ir::rules` (§6.4);
4. `@since`, the labels and the deprecation reason.

New hover targets: struct fields (which show only an ordinal today), enum
values, enumset bits, union arms, parameters, and the rsdl declarations and
lines (in `crates/ridl-lsp/src/rsdl.rs`). A parameter with no doc shows its
type's doc under the line "From `TypeName`:". Hover on a link inside a doc
comment shows the target's hover.

### 7.2 Completion

- Every completion item carries `documentation`: the rendered doc without the
  Contract list, to keep the item short.
- Inside a doc comment, `[` and `.` trigger completion over what the resolver of
  §6.1 can reach: names in scope, then dependable packages, then the members of
  a declaration after a `.`.
- `@` at the start of a doc line completes `@see` and `@since`.

### 7.3 Signature help

Not in 2a. The built grammar has no call expression (`call_expr` is expr-core
V2, roadmap E5.1), so signature help has no call site to serve. Parameter docs
reach the user through hover and completion. Signature help is added with E5.1.

### 7.4 Navigation

Go-to-definition and find-references work on doc links and `@see` targets.
Rename updates every doc link and `@see` that resolves to the renamed item.

### 7.5 Quick fixes

- On `missing-docs`: insert a `///` line above the item, at its indentation.
- On `unknown-doc-tag` for `@deprecated` or `@labels`: remove the tag and add
  the equivalent attribute to the item's attribute block, creating the block
  when there is none.
- On `doc-comment-style`: rewrite a `/** */` comment as `///` lines.

### 7.6 Lint levels and freshness

Levels apply as ADR-0024 decision 6 states for the language server. With §8, a
file inside a member gets the root's `[lints]`. Docs, links and rules come from
the same compile as the diagnostics, so hover never shows a doc that differs
from what `ridl check` reads.

## 8. Root discovery from a member (#529)

### 8.1 The rule

When the nearest `ridl.toml` above the entry is a `[package]` manifest, the
loader keeps walking up:

- If it finds a `[workspace]` manifest whose `members` names the package's
  directory, after the glob expansion and path normalisation the workspace
  already applies, that workspace is the root.
- The walk stops at the first `[workspace]` manifest whether or not it names the
  package, at a directory that holds a VCS root (`.git`), and at the filesystem
  root.
- A package that no workspace names stays standalone, as today.

One function in `ridl-core` implements the rule. `ridl check`, `ridl build`, the
MCP path mode and the language server call it, so every entry point agrees
(ADR-0024 decision 3's reason). The commands that do not report diagnostics
(ADR-0024 decision 8) use it too, so a member resolves its siblings everywhere.

The root's `[lints]`, `[defaults].timing` and `[imports]` then apply to a member
entry. This changes behaviour, and the CHANGELOG entry says so.

### 8.2 What is checked

The checked set does not change: `ridl check members/a` still reports
diagnostics only for files under `members/a`, but it compiles and resolves
through the whole workspace. Reporting on sibling members would be a second
behaviour change that nobody asked for.

### 8.3 The duplicate notice

`shown_load_error` in the language server is set by the first notice from either
path, `initialize` or the first `didOpen`, so one load failure is shown once
(gap 2 of #529).

### 8.4 Records

ADR-0002 §4 gains the rule. ADR-0024 decision 9 ("entering at a workspace member
loads the member alone") is replaced by "entering at a workspace member loads
its workspace, and reports on the member". The book's workspace chapter and the
CLI reference describe it.

## 9. Testing

- **Each lint**: a fixture that raises it and one that does not, through the
  shared corpus harness, plus a `[lints]` fixture that sets it to `deny`.
- **`missing-docs`**: a coverage matrix with one fixture item per kind of §5.2 —
  covered, exempt by `internal`, member of an `internal` declaration, parameter,
  `reserved`, rsdl declaration, rsdl member line; a tags-only doc counts as
  missing; a dependency outside the entry's tree is not reported.
- **Allow by default**: `doc-comment-style` is silent with no `[lints]`, raised
  at `warn`, and Error at `deny`; the SARIF rule has level `none`; the catalogue
  guard rejects an `allow` default on an Error row.
- **Links**: resolver tests for a bare name, an aliased import, a qualified name
  without an import, a member, `internal` in another package, `internal` in the
  same package, a candidate in a code span, a candidate in a fenced block,
  `[0..250]`, `[see below]`, and a name with a reference definition.
- **Tags**: `@see` resolved and broken, `@since` well formed and malformed, an
  unknown tag, `@deprecated` and `@labels` with the hint, `@` mid-line.
- **Promotion**: `deprecated` and `labels` reach the IR on every carrier;
  `fixed` accepts a block with only these keys and rejects any other key
  (RIDL-106); `ridl diff` reports the two categories and `DocOnly`.
- **IR**: a snapshot of a fixture with a doc on every carrier of §3.1; the
  deletion test of §6.3.
- **Rules**: one test per `Rule` variant, and the named-type case.
- **Root discovery**: an editor fixture where a file in member A imports member
  B; `ridl check members/a` applying the root's `deny`; a `[workspace]` that
  does not name the package leaves it standalone; the walk stops at `.git`; the
  checked set of a member entry stays the member.
- **Language server**: hover on each new target, the parameter fallback,
  completion documentation, link completion after `[` and `.`, tag completion,
  go-to-definition and rename on a link, each quick fix, one load-error notice.
- **Book**: the new chapter's fences compile under `book_examples.rs`, one
  `allow=` fence per doc lint.

The review's tests seat mutates and reruns, as for earlier stages.

## 10. Documentation

- A new book chapter, "Documenting your API": house style, carriers, links,
  tags, the `# Examples` convention, `deprecated` and `labels`, and the doc
  lints.
- `docs/book/lints.md`: rows for the new lints and a default-level column.
- The book's examples and `examples/` use `///`, and no `@deprecated` or
  `@labels` tag remains in them.
- The CLI reference and the workspace chapter: root discovery from a member.
- The records of §4.

## 11. Order

The plan refines this; the first cut is:

1. ADR-0025 and the allow-by-default catalogue row.
2. Root discovery (#529).
3. Carriers and scanning: syntax, checker, IR.
4. The promotion and its diff categories.
5. The link resolver and TYPL-401.
6. The remaining doc lints.
7. `ridl_ir::rules`.
8. Language server: hover and completion.
9. Language server: navigation and quick fixes.
10. The book, the examples and the specification records.

Tasks 2, 3 and 4 are independent of each other.

## 12. Alternatives considered

- **#529 in the language server only** (D-1). Less risk to the CLI, but the
  editor and `ridl check` would report different levels for one file, which
  ADR-0024 decision 3 exists to prevent. **Leaving #529 out** keeps cross-member
  links and the root's levels unavailable in the editor.
- **rsdl not covered by `missing-docs`** (D-2). Avoids system IR changes now,
  but leaves the apex of the family undocumented. **Requiring a package doc**
  needs a rule for which of a package's files holds it, and a diagnostic for
  two.
- **Requiring parameter docs** (D-3, the brief's wording). Signature help and 2b
  would always have a per-parameter doc, but R6 makes every parameter type a
  named, documented type, so the doc would mostly repeat it. **No parameter
  carrier** (a `# Parameters` section) leaves hover on a parameter with nothing
  of its own.
- **Keeping `@deprecated` and `@labels` as synonyms for one release** (D-4).
  Gentler for existing sources, at the cost of two spellings and two code paths.
  **Only `@see`, no promotion** leaves the deletion-test conflict open, and 2b's
  generated `#[deprecated]` would come from a doc tag. **`@example`** duplicates
  a Markdown heading and a fenced block.
- **Qualified links only to imported packages** (D-5). A simpler resolver, but a
  doc link would force an import the code does not use. **Raw link text in the
  IR**, resolved by each backend, repeats the resolver in every backend, and the
  out-of-repo Kotlin plugin cannot do it.
- **Hover formats the rules itself** (D-6). Less work now, but 2b would write a
  second formatter that can disagree with the editor. **Defining the IR's rule
  data in 2a** takes a decision that 2b and E16 should take together while E16
  still changes the descriptor IR.
- **Signature help on declarations.** Signature help exists for call sites;
  there are none until E5.1.
- **A `doc-comment-style` option to require `/** */`.** `[lints]` has no lint
  options; adding them is a registry change for one lint.
