# Documentation in the source (spec 2a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every doc comment reaches the IR and the language server, the doc
lints report missing, misplaced and broken docs, and the loader resolves a
workspace member through its workspace (#529).

**Architecture:** `ridl-sem` reads docs from every carrier with a
CommonMark-aware scanner, resolves links with a resolver lifted out of
`ridl-lsp`, and runs a new doc-lint pass beside the existing lint pass. The IR
gains additive doc fields that the catalog hash clears. `ridl-ir` gains a
`rules` module that hover renders. `ridl-core` gains allow-by-default lint rows
and a root discovery that walks from a member to its workspace.

**Tech Stack:** Rust (pin in `rust-toolchain.toml`), rowan syntax trees, prost /
protox / pbjson for the IR, `pulldown-cmark` (already a workspace dependency),
`lsp-server` / `lsp-types`, insta.

**Spec:** `docs/wip/2026-10-04-docs-in-source-design.md` (read it with this
plan; section numbers below, written §N, refer to it).

## Global Constraints

- Prose in comments, commits and docs is plain and literal English (AGENTS.md).
- Conventional Commits with a scope listed in `.git-std.toml`; never push to
  `main`.
- A new lint adds, in the same commit: its catalogue row with `lint = "<name>"`
  in `crates/ridl-core/src/diag.rs`, its row in the table of
  `docs/book/lints.md` (`| Lint | Code | Default | Summary |`), and its entry in
  the expected list of
  `lint_names_are_present_exactly_on_warnings_and_infos_and_unique` (ADR-0024
  decision 7).
- Lint names and codes are exactly those of spec §5.1: TYPL-401
  `broken-doc-link`, TYPL-404 `detached-doc-comment`, TYPL-405
  `deprecated-without-reason` (unchanged), TYPL-406 `missing-docs`, TYPL-407
  `misplaced-doc-comment`, TYPL-408 `unknown-doc-tag`, TYPL-409
  `malformed-doc-tag`, TYPL-410 `doc-comment-style` (default `allow`). All
  Warning severity.
- Proto changes add fields only, with the next free number in each message,
  never a number listed under `reserved` (ADR-0014).
- The only new dependency is `pulldown-cmark` for `ridl-sem`, from the workspace
  entry (`default-features = false`). `just wasm-check` must pass.
- A source comment cites ADR-0025, ADR-0024 or a language reference section,
  never `docs/wip/` (those paths die when the spec is archived).
- `@deprecated` and `@labels` keep today's behaviour (spec D-4, §3.4).
- `just verify` passes before the PR.

## Review Focus

1. A `/** */` doc with `*` line decoration: tags and links are found after the
   decoration is stripped, and their source spans point at the right columns
   (test in Task 4).
2. CRLF line endings in a doc comment: tag detection and link byte offsets are
   correct (test in Task 4).
3. Multi-byte UTF-8 before a link: the IR `offset`/`len` are byte offsets into
   `doc`, and the language server converts the source span to the right UTF-16
   position (tests in Task 5 and Task 9).
4. A member opened in the editor while the root `ridl.toml` does not parse: the
   load error is shown once and names the root manifest (test in Task 2).
5. Renaming a type that a doc link in a sibling member names: the link is
   updated (test in Task 9).

---

### Task 1: Allow-by-default lint rows and `doc-comment-style`

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (macro `diag_codes!` at :113, the row
  type near :121, TYPL rows near :523, test at :1845)
- Modify: `crates/ridl-core/src/lint.rs` (`default_level` at :87)
- Modify: `crates/ridl-core/src/diag/sarif.rs` (`rule` at :157)
- Create: `crates/ridl-sem/src/doc_lint.rs`
- Modify: `crates/ridl-sem/src/lib.rs`, `crates/ridl-sem/src/check.rs` (call
  beside `lint::lint_package` at :221), `crates/ridl-sem/src/rsdl/mod.rs`
  (`check_system` at :53)
- Modify: `crates/ridl/tests/book_lints.rs` only if it cannot read an `allow`
  default; `docs/book/lints.md`
- Modify: `docs/decisions/ADR-0024-lint-registry-and-levels.md` (decisions 1 and
  15, plus a `## Status` amendment line)

**Interfaces:**

- Produces: row syntax `..., lint = "<name>", default = allow;`;
  `CatalogEntry.allow_by_default: bool`; `default_level` returns
  `Some(LintLevel::Allow)` for such a row; `ridl_sem::doc_lint` module with
  `pub(crate) fn lint_package(checker: &mut Checker, files: &[…])` (same
  parameter types as `lint::lint_package`) and
  `pub(crate) fn lint_rsdl_file(file: &SourceFile, file_id: FileId) -> Vec<Diagnostic>`.
  Later tasks add checks to both functions.

- [ ] **Step 1: Write the failing tests**

In `diag.rs` tests:

```rust
#[test]
fn allow_by_default_is_only_on_warning_or_info_rows() {
    for entry in ALL_CATALOGS.iter().flat_map(|c| c.iter()) {
        if entry.allow_by_default {
            assert_ne!(entry.severity, Severity::Error, "{}", entry.code);
            assert!(entry.lint.is_some(), "{}", entry.code);
        }
    }
}
```

In `lint.rs` tests:
`default_level(lint_by_name("doc-comment-style").unwrap())
== Some(LintLevel::Allow)`;
`default_level` of TYPL-404 stays `Some(Warn)`.

In `sarif.rs` tests: the rule for TYPL-410 has
`defaultConfiguration.level == "none"`; TYPL-404 stays `"warning"`.

In `doc_lint.rs` tests (through `check_source` as `check.rs` tests do):

- `doc_comment_style_flags_block_doc`: `/** A speed. */\ntype S: f32` draws
  `TYPL-410` with a fix-it whose replacement is `/// A speed.`.
- `doc_comment_style_accepts_line_doc`: `/// A speed.` draws nothing.
- `doc_comment_style_multiline_fixit`: a three-line `/** … */` with `*`
  decoration becomes three `///` lines at the comment's indentation.

In `crates/ridl/tests/lints.rs`: `doc_comment_style_is_silent_by_default` (no
`[lints]`, `ridl check` output has no TYPL-410),
`doc_comment_style_at_warn_is_reported`, `doc_comment_style_at_deny_exits_1`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p ridl-core -p ridl-sem doc_comment_style allow_by_default`
and `cargo test -p ridl --test lints doc_comment_style` Expected: compile errors
(no field, no code) or FAIL.

- [ ] **Step 3: Implement**

- Macro: optional `, default = allow` after `lint = …`; any other ident is a
  compile error.
- Row:
  `TYPL_410 = "TYPL-410", Warning, "doc comment written as /** */", lint = "doc-comment-style", default = allow;`
- `sarif.rs`: `level` is `"none"` when `allow_by_default`.
- `doc_lint`: walk the file's tokens; each `DocComment` token whose text starts
  with `/**` draws TYPL-410 at its range, with one `FixIt` replacing the token
  by `///` lines (strip `/**`, `*/`, and a leading `*` plus one space per line;
  drop empty first and last lines).
- Book row:
  ``| `doc-comment-style` | TYPL-410 | allow | doc comment written as `/** */` |``.
- ADR-0024: decision 1 states that a row may declare `allow` as its default;
  decision 15 notes `Some(Allow)` for such rows; Status gains an amendment line
  naming ADR-0025.

- [ ] **Step 4: Run the tests and `cargo test -p ridl --test book_lints`**

Expected: PASS.

- [ ] **Step 5: Commit**

`feat(ridl-core): allow-by-default lint rows and the doc-comment-style lint`

---

### Task 2: Root discovery from a member (#529)

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` (`find_manifest_root` :225,
  callers :184, :189; `LoadedWorkspace`), `crates/ridl-core/src/lib.rs` (:48)
- Modify: `crates/ridlc/src/lib.rs` (`run_check` :617, `run_build_with`)
- Modify: `crates/ridl/src/main.rs` (`index_root` :2336)
- Modify: `crates/ridl-mcp/src/snapshot.rs` (:48-77)
- Modify: `crates/ridl-lsp/src/server.rs` (`load_for_opened_file` :361,
  `shown_load_error` :247/:375/:379, the initialize notice)
- Test: `crates/ridl-core/src/workspace.rs` tests, `crates/ridl/tests/lints.rs`
  (:252), `crates/ridl-lsp/tests/server.rs`, `crates/ridl-mcp` tests
- Docs: `docs/decisions/ADR-0002-module-system.md` §4 (:152-186), ADR-0024
  decision 9 (:132-139) and its Status, `docs/book/lints.md` (:58-75),
  `docs/book/cli-reference.md` (:113), `docs/book/getting-started.md` ("A
  workspace", :81)

**Interfaces:**

- Produces: `pub fn find_root(dir: &Path) -> Option<PathBuf>` in
  `ridl_core::workspace` (re-exported);
  `LoadedWorkspace.report_scope:
  Option<PathBuf>` — the member directory when
  the entry was inside a member reached through its workspace, else `None`;
  `pub fn retain_in_report_scope(diags: &mut Vec<Diagnostic>, sources: &SourceMap, scope: Option<&Path>)`
  in `ridlc`.

- [ ] **Step 1: Write the failing tests**

`workspace.rs` (temp dirs):

- `find_root_walks_from_a_member_to_its_workspace`: `root/ridl.toml`
  `[workspace] members = ["a", "b"]`, `root/a/ridl.toml` `[package]` →
  `find_root(root/a/src) == Some(root)`.
- `find_root_keeps_an_unlisted_package_standalone`: members `["b"]` →
  `Some(root/a)`.
- `find_root_stops_at_the_first_workspace`: an outer workspace above an inner
  workspace that does not list `a` → `Some(root/a)`.
- `find_root_stops_at_a_git_directory`: `root/.git/` between the package and an
  outer workspace that lists it → `Some(root/a)`.
- `member_entry_sets_report_scope`: `load_workspace(root/a)` has
  `report_scope == Some(root/a)` and compiles both members.

`crates/ridl/tests/lints.rs`: rename `member_entry_ignores_root_lints` to
`member_entry_applies_root_lints` and invert it (the root's `deny` makes
`ridl check members/a` exit 1); add `member_entry_reports_only_the_member` (an
error in member `b` does not appear when checking `a`, and the exit code is 0)
and `member_entry_resolves_a_sibling_import`.

`crates/ridl-lsp/tests/server.rs`:

- `a_member_file_resolves_an_import_of_a_sibling_member` (hover on the imported
  type shows its doc).
- `a_member_with_an_unparsable_root_manifest_shows_one_error` (Review Focus 4:
  exactly one `window/showMessage`, naming the root `ridl.toml`).
- `the_initialize_notice_and_did_open_do_not_repeat_a_load_error` (gap 2).

MCP: path mode on a member resolves a sibling import, with no "loaded the
package alone" note.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridl-core find_root member_entry`;
`cargo test -p ridl --test lints member_entry`;
`cargo test -p ridl-lsp --test server member` Expected: FAIL.

- [ ] **Step 3: Implement**

- `find_root`: take `find_manifest_root(dir)`; if that manifest is a
  `[package]`, walk parents; at the first directory with a `ridl.toml` of kind
  `[workspace]`, return it when its `members` (joined and lexically normalised
  as `load_member` joins them) contains the package directory, else return the
  package; stop with the package when a directory contains `.git` (after
  checking that directory's own `ridl.toml`) or at the filesystem root. A
  manifest that fails to parse during the walk stops it and is returned, so the
  loader reports the parse error.
- Replace every `find_manifest_root` caller listed above with `find_root`;
  remove `find_manifest_root` if no caller is left.
- `report_scope`: set when the entry lies under a member directory of the loaded
  workspace.
- `ridlc::run_check` and `run_build_with` call `retain_in_report_scope` before
  levels are applied; diagnostics with no path are kept. The MCP path mode does
  the same. The language server does not filter.
- `shown_load_error`: set by whichever of the initialize notice and the first
  `didOpen` shows the error first.
- Docs: ADR-0002 §4 states the walk; ADR-0024 decision 9 becomes "entering at a
  workspace member loads its workspace and reports on the member", with a Status
  line naming ADR-0025; the book pages describe it.

- [ ] **Step 4: Run the tests, then `cargo test --workspace --locked`**

Expected: PASS. Any other test that assumed member-alone loading is updated to
the new rule, not deleted.

- [ ] **Step 5: Commit**

`fix(ridl-core): load a workspace member through its workspace (#529)`

---

### Task 3: IR doc fields

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`,
  `crates/ridl-ir/proto/ridl/ir/v2/system.proto`
- Modify: `crates/ridl-ir/src/catalog_hash.rs` (`blank_docs` :490, interface
  reduction :70-76)
- Modify: `crates/ridl-diff/src/walk.rs` (`envelope_differs` paths at :95, :445,
  :694, :886; the params comparison of `diff_interaction`),
  `crates/ridl-diff/src/system.rs` (`diff_systems` :59)
- Update snapshots that print the new empty fields only if pbjson prints them
  (it omits defaults; expect none).

**Interfaces:**

- Produces (proto):
  `message DocLink { string text = 1; uint32 offset = 2; uint32 len = 3; string target = 4; }`
  in `ir.proto` (import it in `system.proto` if not already imported). Fields
  `repeated DocLink links`, `repeated DocLink see`, `repeated string since` on
  `Decl`, `Field`, `EnumValue`, `UnionArm`, `Interface`, `Service`, `Param`;
  `string doc` on `Param`; `string doc` plus the same three on `System`,
  `Component`, `Offer`, `Require`, `Distribution`, `Deployment`, `Machine`,
  `MemberLine`.

- [ ] **Step 1: Write the failing tests**

`catalog_hash.rs`: `catalog_hash_ignores_every_doc_field` — build a package with
one of each declaration kind and an interface with a command that has a
parameter, compute the hash, then set `doc`, `links`, `see`, `since` (and
`labels`, `deprecated` where present) to non-empty values on every message that
has them, and assert the hash is unchanged.

`ridl-diff`: `a_parameter_doc_change_is_doc_only` (old/new differ only in
`Param.doc` → one `DocOnly`, no `ParamsChanged`); `a_since_change_is_doc_only`;
`system_doc_changes_are_not_reported` (`diff_systems` returns no change).

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridl-ir catalog_hash_ignores_every_doc_field`;
`cargo test -p ridl-diff doc_only system_doc` Expected: compile errors (no
fields), then FAIL.

- [ ] **Step 3: Implement**

Add the proto fields; clear them in `blank_docs` and the interface reduction;
include them in the envelope comparisons so a change is `DocOnly`; exclude
`Param.doc`, `links`, `see`, `since` from the parameter comparison; make
`diff_systems` ignore the system doc fields.

- [ ] **Step 4: Run the tests and
      `cargo test -p ridl-ir -p ridl-diff -p ridlc`**

Expected: PASS.

- [ ] **Step 5: Commit**

`feat(ridl-ir): doc, link, see and since fields on every doc carrier`

---

### Task 4: Every carrier read, and the positional and tag lints

**Files:**

- Modify: `crates/ridl-syntax/src/ast.rs` (:395-417)
- Modify: `crates/ridl-sem/Cargo.toml` (`pulldown-cmark.workspace = true`)
- Modify: `crates/ridl-sem/src/docs.rs`
- Modify: `crates/ridl-sem/src/check.rs` (`lower_definition` :945, TYPL-404 and
  TYPL-405 at :981/:988, `lower_field` :2217, `lower_enum` :2938,
  `lower_enum_set` :3052, `lower_union` :3188, `lower_interaction` :3689,
  `lower_service` :3746, `lower_params` :5091)
- Modify: `crates/ridl-sem/src/rsdl/lower.rs` (`lower_system` :32 and its
  builders) and whatever carries the rsdl syntax nodes to it
- Modify: `crates/ridl-sem/src/doc_lint.rs`
- Modify: `crates/ridl-core/src/diag.rs`, `docs/book/lints.md` (TYPL-407,
  TYPL-408, TYPL-409 rows; TYPL-404 summary widened)

**Interfaces:**

- Consumes: Task 3's proto fields; Task 1's `doc_lint` entry points.
- Produces:

```rust
pub struct DocInfo {
    pub doc: String,                 // markers stripped, tag lines removed
    pub labels: Vec<String>,         // unchanged
    pub deprecated: Option<String>,  // unchanged
    pub links: Vec<LinkCandidate>,
    pub see: Vec<LinkCandidate>,
    pub since: Vec<String>,
    pub problems: Vec<TagProblem>,
}
pub struct LinkCandidate {
    pub segments: Vec<String>,       // ["pkg", "Name", "member"]
    pub text: String,                // the visible link text
    pub doc_range: std::ops::Range<usize>, // bytes in DocInfo::doc; empty for see
    pub source: TextRange,           // span in the source file
}
pub struct TagProblem { pub kind: TagProblemKind, pub source: TextRange }
pub enum TagProblemKind { Unknown(String), Malformed(&'static str) }
```

`docs::scan(tokens: &[SyntaxToken]) -> DocInfo` keeps its signature.

- [ ] **Step 1: Write the failing tests**

`docs.rs` unit tests (scan only):

- `scan_finds_three_link_forms`: `[Speed]`, ``[`veh.Gear`]``,
  `[the gear][Gear.PARK]` → segments `["Speed"]`, `["veh","Gear"]`,
  `["Gear","PARK"]`.
- `scan_ignores_non_identifier_brackets`: `[0..250]`, `[see below]` → no links.
- `scan_ignores_code`: `` `[Speed]` `` and a fenced block containing `[Speed]` →
  no links.
- `scan_ignores_reference_definitions`: `[Speed]` with `[Speed]: https://x` in
  the same doc → no link.
- `scan_reads_tags`: `@see veh.Gear`, `@since 1.2`, `@since 1.2.3` → one see,
  two since; `@sinc 1.0` → `Unknown("sinc")`; `@since soon` and a bare `@see` →
  `Malformed("since")`, `Malformed("see")`; `mail me @home` → nothing.
- `scan_block_doc_decoration` (Review Focus 1):
  `/**\n * [Speed] here\n * @since 1.0\n */` → one link whose `source` covers
  exactly `[Speed]`, one since.
- `scan_crlf` (Review Focus 2): the same doc as `///` lines joined by `\r\n` →
  same result, `doc_range` slices `doc` to `[Speed]`.

`check.rs` / `doc_lint.rs` tests:

- `member_docs_reach_the_ir`: a struct field, an enum value, an enumset bit, a
  union arm and a command parameter, each with `/// Doc N.` → each IR `doc` is
  `Doc N.`.
- `since_reaches_the_ir`: `@since 1.2` on a struct → `decl.since == ["1.2"]`.
- `rsdl_docs_reach_the_system_ir`: a doc on a system, a component, an `offers`
  line, a deployment and a machine → each IR message's `doc`.
- `detached_doc_on_a_field` (TYPL-404 on a member).
- `misplaced_doc_comment_positions` (TYPL-407): one fixture each for a doc above
  `package`, above `import`, before a return type, at end of file → one TYPL-407
  each; a doc above a field → none.
- `unknown_and_malformed_tags` (TYPL-408, TYPL-409), at the tag's span.
- `deprecated_and_labels_unchanged`: the existing TYPL-405 test still passes and
  `@labels A, B` still fills `decl.labels`.

- [ ] **Step 2: Run and see them fail**

Run:
`cargo test -p ridl-sem scan_ member_docs since_ rsdl_docs misplaced unknown_and detached_doc`
Expected: FAIL.

- [ ] **Step 3: Implement**

- `impl HasDocComments` for `FieldDef`, `EnumValue`, `EnumSetBit`, `UnionArm`,
  `Param`, `ComponentLine` (if missing). Confirm each attaches with a parser
  test; if the doc trivia of a member lands outside the node's sibling run, fix
  the walk in `doc_comments()`, not the parser.
- `scan`: strip markers per line, keep a per-line map from `doc` byte offsets to
  source offsets, run `pulldown-cmark` (offset iterator) to find text outside
  code; match `[…]`, ``[`…`]`` and `[…][…]` whose target matches
  `^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$`; tag lines are lines
  whose first non-space character is `@`; `@since` value matches
  `^\d+\.\d+(\.\d+)?$`; `@see` value is one qualified name.
- Lowering fills `doc` and `since` for each new carrier; `links` and `see` are
  filled in Task 5.
- Move TYPL-404 out of `lower_definition` into `doc_lint`, applied to every
  carrier; TYPL-405 stays where it is.
- TYPL-407: a `DocComment` token whose next non-trivia sibling is not a carrier
  node, or that has none.
- Catalogue rows TYPL-407/408/409 with the spec §5.1 summaries, book rows.

- [ ] **Step 4: Run the tests and `cargo test --workspace --locked`**

Expected: PASS. Book fences or fixtures that now draw TYPL-407/408 are fixed in
the source text (move or remove the doc), not filtered.

- [ ] **Step 5: Commit**

`feat(ridl-sem): read docs on every carrier, with the positional and tag lints`

---

### Task 5: Doc links and `@see` resolve (TYPL-401)

**Files:**

- Modify: `crates/ridl-sem/src/resolve.rs`
- Modify: `crates/ridl-lsp/src/nav.rs` (`resolve_reference` :165 becomes a thin
  call to the new function)
- Modify: `crates/ridl-sem/src/check.rs`, `crates/ridl-sem/src/rsdl/lower.rs`
  (fill `links` and `see`), `crates/ridl-sem/src/doc_lint.rs`
- Modify: `crates/ridl-core/src/diag.rs`, `docs/book/lints.md` (TYPL-401 row)

**Interfaces:**

- Consumes: `LinkCandidate` (Task 4), `Resolution`/`Symbol` (`resolve.rs`
  :44-64), `resolve_package` (:89).
- Produces:

```rust
pub struct LinkTarget { pub symbol: Symbol, pub member: Option<String> }
impl LinkTarget { pub fn canonical(&self) -> String } // "pkg.Name" or "pkg.Name.member"
pub enum LinkError { Unresolved, NotVisible, NoSuchMember }
pub fn resolve_doc_link(
    db: &dyn Db, ws: &Workspace, std: Option<&Package>, pkg: &Package,
    resolution: &Resolution, segments: &[String],
) -> Result<LinkTarget, LinkError>
```

Use the exact parameter types `nav::resolve_reference` uses today.

- [ ] **Step 1: Write the failing tests** (in `resolve.rs` or `doc_lint.rs`,
      multi-package workspaces as the existing resolver tests build them)

- `link_bare_local`, `link_through_alias` (`import veh.Gear as G`; `[G]`
  resolves to `veh.Gear`), `link_qualified_without_import` (`[veh.Gear]` with
  `veh` a workspace member and no import), `link_to_member` (`[Gear.PARK]`,
  `[Pose.x]`, `[Cruise.setLever]`), `link_no_such_member` → TYPL-401,
  `link_internal_other_package` → TYPL-401, `link_internal_same_package` →
  resolves, `link_to_std` (`[Timestamp]` or any std type).
- `see_resolves_and_reaches_the_ir`: `@see veh.Gear` →
  `decl.see[0].target ==
  "veh.Gear"`.
- `links_reach_the_ir_with_byte_offsets` (Review Focus 3): doc
  `/// Größe in
  [Speed].` → `links[0].offset` equals the byte index of `[` in
  `decl.doc`, `len == 7`, `target == "app.Speed"`.
- `unresolved_link_is_not_stored`.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridl-sem link_ see_resolves links_reach` Expected: FAIL.

- [ ] **Step 3: Implement**

Lift the body of `nav::resolve_reference`; extend qualified lookup to every
package reachable under ADR-0002 §5; add the member segment over the IR
declaration's fields, enum values, enumset bits, union arms and interactions;
reject `internal` across packages. The checker resolves every candidate and
every `see`, stores resolved ones in the IR, and raises TYPL-401 at the
candidate's `source` span for each error, with the message naming the reason.

- [ ] **Step 4: Run the tests and `cargo test --workspace --locked`**

Expected: PASS. Existing docs in the book, `examples/`, `ridl_std.typl` and
fixtures that now draw TYPL-401 are corrected in their text.

- [ ] **Step 5: Commit**

`feat(ridl-sem): resolve doc links and @see targets (TYPL-401)`

---

### Task 6: `missing-docs` (TYPL-406)

**Files:**

- Modify: `crates/ridl-sem/src/doc_lint.rs`, `crates/ridl-core/src/diag.rs`,
  `docs/book/lints.md`
- Modify: test helpers — `codes` in `crates/ridl-sem/src/check.rs` (:6101) and
  the equivalent helpers of any suite that asserts an exact diagnostic list;
  `crates/ridl/tests/book_examples.rs`
- Modify: `crates/ridl-core/assets/ridl_std.typl` (the eight `*_PATTERN` consts,
  lines 5-12), `examples/cabin` sources

**Interfaces:**

- Consumes: Task 4's carriers, `HasModifiers::is_internal`.
- Produces: TYPL-406 at the item's name span; `codes_all(checked)` in `check.rs`
  tests (unfiltered) next to `codes` (which now drops TYPL-406).

- [ ] **Step 1: Write the failing tests** (`doc_lint.rs`)

`missing_docs_matrix` — one package, asserting the exact set of names reported:

| Item                                            | Reported      |
| ----------------------------------------------- | ------------- |
| undocumented `struct Pose` and its field `x`    | both          |
| `internal struct Hidden` and its field          | neither       |
| documented `enum Gear` with undocumented `PARK` | `PARK`        |
| undocumented enumset bit, union arm             | both          |
| `interface Cruise` and its `command set(v: V)`  | both, not `v` |
| `reserved` entry                                | no            |
| a doc with only `@since 1.0`                    | yes           |

`missing_docs_rsdl`: undocumented `system`, `component`, `deployment`,
`machine`, `distribution` → each reported; an undocumented `offers` line → not.

`missing_docs_quick_fix`: the fix-it inserts `/// \n` plus the item's
indentation at the start of the item's line.

`crates/ridl/tests/lints.rs`: `missing_docs_skips_dependencies` (a workspace
importing a dependency package outside its tree with undocumented items reports
nothing for it); `missing_docs_at_allow_is_silent`.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p ridl-sem missing_docs`;
`cargo test -p ridl --test lints missing_docs` Expected: FAIL.

- [ ] **Step 3: Implement**

Coverage exactly as spec §5.2. Then make the suites quiet: `codes` drops
TYPL-406; the book harness ignores TYPL-406 in a fence unless the fence names
`allow=TYPL-406`; any other harness that compares exact diagnostic lists drops
TYPL-406 in its helper. Document the eight std consts and every item in
`examples/cabin`, so `just demo` and a check of the cabin workspace report no
TYPL-406.

- [ ] **Step 4: Run `cargo test --workspace --locked` and `just demo`**

Expected: PASS; `ridl check examples/cabin` reports no TYPL-406.

- [ ] **Step 5: Commit**

`feat(ridl-sem): the missing-docs lint (TYPL-406)`

---

### Task 7: `ridl_ir::rules`

**Files:**

- Create: `crates/ridl-ir/src/rules.rs`; modify `crates/ridl-ir/src/lib.rs`

**Interfaces:**

- Produces:

```rust
pub enum ItemRef<'a> {
    Decl(&'a v2::Decl), Field(&'a v2::Field), Param(&'a v2::Param),
}
pub enum Rule {
    Range { min: Option<String>, max: Option<String> },
    Step(String),
    Unit(String),
    Length { min: Option<u64>, max: Option<u64> },       // Constraint.len_min/len_max
    Pattern(String),
    CollectionBound { min: u64, max: Option<u64> },      // ArrayType / MapType
    Timing { mode: v2::TimingMode, min_us: Option<u64>, max_us: Option<u64> },
    ResponseBound { min_us: Option<u64>, max_us: Option<u64> },
    Errors(Vec<String>),                                  // qualified error type names
    Contract { kind: v2::ContractKind, source: String },
}
pub fn rules(pkg: &v2::Package, deps: &[&v2::Package], item: ItemRef<'_>) -> Vec<Rule>;
pub fn render(rule: &Rule) -> String;
```

Module doc states the shape is provisional and an input to spec 2b.

- [ ] **Step 1: Write the failing tests** — one per variant from small IR built
      by `ridl_sem` test helpers or by hand: a `type S: f32 m [0..250 step 0.5]`
      gives `[Range{0,250}, Step("0.5"), Unit("m")]`; a bounded string type
      gives `Length`; an array field gives `CollectionBound`; a signal gives
      `Timing`; a command with a declared bound gives `ResponseBound`; a query
      returning `T | E` gives `Errors(["app.E"])`; a `require` gives `Contract`;
      a field of named type `S` returns `S`'s rules
      (`field_follows_named_type`); `render` of each variant gives a fixed
      string (assert the exact strings you choose, e.g. `range 0 ..= 250`).

- [ ] **Step 2: Run and see them fail** — `cargo test -p ridl-ir rules`

- [ ] **Step 3: Implement** — read the IR only; no IR change.

- [ ] **Step 4: Run and pass** — `cargo test -p ridl-ir`

- [ ] **Step 5: Commit** —
      `feat(ridl-ir): rule extraction for hover and spec 2b`

---

### Task 8: Hover and completion documentation

**Files:**

- Modify: `crates/ridl-lsp/src/hover.rs` (`hover` :52, `field_hover` :85,
  `render_decl` :172, `render_interaction` :491, `service_hover` :425),
  `crates/ridl-lsp/src/rsdl.rs` (`hover` :24), `crates/ridl-lsp/src/complete.rs`
  (`item` :364)
- Test: `crates/ridl-lsp/tests/server.rs` (helpers `open_and_hover` :409,
  `complete_at` :1047)

**Interfaces:**

- Consumes: `ridl_ir::rules::{rules, render, ItemRef}` (Task 7); IR doc and link
  fields (Tasks 3-5).
- Produces:
  `fn render_doc(doc: &str, links: &[v2::DocLink], resolve: impl Fn(&str) -> Option<lt::Location>) -> String`
  in `hover.rs`, reused by Task 9.

- [ ] **Step 1: Write the failing tests**

- `hover_on_a_field_shows_doc_and_contract`: field `speed: Speed` with
  `/// Current speed.` → markdown contains `Current speed.` and a `Contract`
  section with the range of `Speed`.
- `hover_on_an_enum_value_union_arm_and_parameter`.
- `hover_on_a_parameter_without_doc_falls_back_to_its_type` → contains
  ``From `Speed`:`` and `Speed`'s doc.
- `hover_renders_a_doc_link_as_a_markdown_link` → `[Gear](file://…#L…)`.
- `hover_shows_since_labels_and_deprecation_in_order`.
- `hover_on_an_rsdl_component_shows_its_doc`.
- `completion_items_carry_documentation`: completing a type name returns an item
  whose `documentation` is the type's doc and contains no `Contract`.
- Review Focus 3: `hover_link_after_multibyte_text` → the link's target line is
  right for a doc `/// Größe: [Speed]`.

- [ ] **Step 2: Run and see them fail** —
      `cargo test -p ridl-lsp --test server hover_ completion_items`

- [ ] **Step 3: Implement** the order of spec §7.1 (signature, doc, Contract,
      since/labels/deprecation) for every hover target.

- [ ] **Step 4: Run and pass** — `cargo test -p ridl-lsp`

- [ ] **Step 5: Commit** —
      `feat(ridl-lsp): docs and contracts on every hover and completion item`

---

### Task 9: Doc links in the editor

**Files:**

- Create: `crates/ridl-lsp/src/doc.rs` (cursor inside a doc comment)
- Modify: `crates/ridl-lsp/src/complete.rs`, `nav.rs`, `rename.rs`, `server.rs`
  (completion trigger characters add `[` and `@`)
- Test: `crates/ridl-lsp/tests/server.rs`

**Interfaces:**

- Consumes: `docs::scan`, `resolve_doc_link` (Tasks 4-5), `render_doc` (Task 8).
- Produces:
  `pub(crate) fn doc_link_at(file: &SourceFile, offset: TextSize) -> Option<LinkCandidate>`
  and
  `pub(crate) fn in_doc_comment(file: &SourceFile, offset: TextSize) -> Option<DocCursor>`
  where
  `DocCursor { after_open_bracket: Option<String>, at_line_start_at: bool }`.

- [ ] **Step 1: Write the failing tests**

- `completion_after_bracket_in_a_doc_lists_reachable_names` (local, imported, a
  qualified workspace package).
- `completion_after_dot_in_a_doc_link_lists_members`.
- `completion_after_at_on_a_doc_line_lists_tags` → exactly `see`, `since`,
  `deprecated`, `labels`.
- `goto_definition_on_a_doc_link` and on an `@see` target.
- `references_include_doc_links`.
- `rename_updates_doc_links`; Review Focus 5:
  `rename_updates_a_doc_link_in_a_sibling_member`.
- `hover_on_a_doc_link_shows_the_target`.

- [ ] **Step 2: Run and see them fail** —
      `cargo test -p ridl-lsp --test server doc_link completion_after rename_updates goto_definition_on_a_doc references_include`

- [ ] **Step 3: Implement** using `docs::scan` on the comment token under the
      cursor; navigation and rename go through `resolve_doc_link`, so they agree
      with TYPL-401.

- [ ] **Step 4: Run and pass** — `cargo test -p ridl-lsp`

- [ ] **Step 5: Commit** —
      `feat(ridl-lsp): completion, navigation and rename for doc links`

---

### Task 10: Records and the book

**Files:**

- Create: `docs/decisions/ADR-0025-doc-comments.md`, `docs/book/documenting.md`
- Modify: `docs/decisions/README.md`, `docs/book/SUMMARY.md` (after "Describing
  a system"), `docs/specification/typl-language-reference.md` §14 and §16,
  `docs/specification/ridl-language-reference.md` and
  `docs/specification/rsdl-language-reference.md` (doc-comment notes),
  `docs/wip/family-general-form.md` §4.7 (not implemented, deferred to 2b),
  `docs/specification/ridl-family-overview.md` (§2, §5, §6, §7 per its footer at
  :343; ADR list), `docs/book/getting-started.md` (`/**` blocks to `///`),
  `AGENTS.md` (ADR-0025 line in the reading map), `docs/ROADMAP.md` (Epic 8
  landed paragraph, as Spec 0 has)

- [ ] **Step 1: Write ADR-0025** with sections Status, Context, Decision (D-3,
      D-4, D-5, the carrier table §3.1, link forms §3.2, tags §3.3, the deferral
      §3.4, the allow-by-default row §5.3 and the #529 rule §8 as the amendments
      it makes), Alternatives considered (spec §12), Consequences, References;
      include the ADR-0011/0012/0015 checks of spec §4.

- [ ] **Step 2: Write `documenting.md`** — house style, carriers, links, the
      four tags, `# Examples`, the doc lints with one
      `` ```ridl,allow=TYPL-406 `` fence and fully documented fences otherwise;
      packages named `docs.*` so they do not collide with other chapters.

- [ ] **Step 3: Update the specifications and indexes listed above.**

- [ ] **Step 4: Verify** — `just book-check`, `just link-check`,
      `just doc-path-check`,
      `cargo test -p ridl --test book_examples --test book_lints`, `just check`.
      Expected: all pass.

- [ ] **Step 5: Commit** — `docs(docs): ADR-0025 and the documenting chapter`

Then run `just verify` on the branch before opening the PR.
