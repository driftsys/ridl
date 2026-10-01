# `ridl fmt` layout for the ridl and rsdl declarations — implementation plan

Status: implementation plan, written 2026-10-01 for driftsys/ridl#387. Not
started. It is written for a coding agent that works through a shell and git and
starts with only this repository and this file.

## 1. Goal and scope

`ridl fmt` emits seven declarations exactly as written: ridl `interface` and
`service`, and rsdl `system`, `component`, `distribution`, `deployment` and
`machine`. This plan gives each of them the canonical layout of the design note
[`fmt-ridl-rsdl-layout.md`](fmt-ridl-rsdl-layout.md), adds the line width of
note §6 (100 columns, `max_line_length` from `.editorconfig` overriding it) to
every declaration, and reformats the fixtures, `examples/` and the book fences
to the result. The note is the normative source: where this plan and the note
disagree, the note wins, and the disagreement is reported to the maintainer.

Out of scope: everything the note defers — the D-10 follow-up that reformats the
fences of the language references (`docs/specification/`) and of
`docs/wip/family-general-form.md`, including the erratum in ridl reference
Appendix C (D-4) and the two-space `ensure` examples (D-5); breaking an
expression across lines (note §6.3); reading `indent_size` or `indent_style`
(D-12); any change to the grammar, the parser or a diagnostic.

## 2. Read first

- `AGENTS.md` — the repository rules, the `just` recipes, the commit
  conventions.
- `docs/wip/fmt-ridl-rsdl-layout.md` — the whole note. §8 lists the thirteen
  decisions, all decided; the plan cites them as D-1 to D-13.
- `docs/wip/family-general-form.md` §5 — the formatting rules the note extends;
  §4.4 and R5 for the attribute block and the annotation order.
- `docs/technotes/walking-skeleton-architecture.md` — the `crates/ridl-fmt`
  entry, for where the formatter sits.
- `crates/ridl-fmt/src/lib.rs` — the whole formatter. The functions that matter:
  `format` (the entry point), `layout_container` (one pass over a body: blocks,
  comments, blank lines), `blanks_between`, `block_kind`, `format_element` (the
  dispatch by node kind; its last arm, `_ => line(node.text().to_string())`, is
  the verbatim fallback six of the seven declarations reach today; a `machine`
  occurs only inside a `deployment` and is emitted as part of its text),
  `format_block_def`, `block_header_prefix`, `split_brace_line_comment`,
  `field_type`, `is_field_type`, `format_field_type`, `format_constraint`,
  `format_reserved_entry`, `tight_text`, `is_single_line_element`,
  `has_direct_comment`. The module documentation at the top states the rules in
  force.
- `crates/ridl-fmt/tests/` — `corpus.rs` (golden pairs in
  `crates/ridl-fmt/test_data/input` and `formatted`, `.typl` only today),
  `properties.rs` (`content_tokens`,
  `formatting_is_idempotent_over_the_ok_corpus`,
  `content_tokens_detect_a_dropped_comment_and_a_mutated_literal`), `rsdl.rs`
  (`an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written`,
  rewritten by D-1).
- `crates/ridl-syntax/family.ungram` — the grammar of every node the note names.
  `crates/ridl-syntax/src/syntax_kind.rs` — the node kinds: `InterfaceDef`,
  `SignalDef`, `EventDef`, `CommandDef`, `QueryDef`, `FixedDef`,
  `ReservedEntry`, `Param`, `ParamList`, `ReturnType`, `StreamType`,
  `FallibleType`, `InitValue`, `Timing`, `TimingRange`, `AttrBlock`,
  `Attribute`, `AttrValue`, the six expression kinds (`BinaryExpr`,
  `PrefixExpr`, `MemberExpr`, `PathExpr`, `ParenExpr`, `LiteralExpr`),
  `ServiceDef`, `DottedName`, `SystemDef`, `ComponentDef`, `ComponentLine`,
  `DistributionDef`, `DeploymentDef`, `MachineDef`, `MemberLine`, `Reference`.
  `crates/ridl-syntax/test_data/parser/ok` and `err` — the parser corpus.
- `crates/ridl/src/main.rs` — `run_fmt` (the CLI; formats every `.typl`, `.ridl`
  and `.rsdl` file under a path) and the module comment at line 6, which still
  says `.typl` only.
- `crates/ridl-lsp/src/server.rs` — the `formatting` handler, the other caller
  of `ridl_fmt::format`.
- `crates/ridl/tests/facade.rs` — the CLI `ridl fmt` tests
  (`fmt_formats_an_rsdl_file` and its neighbours).
- `crates/ridl-lsp/tests/server.rs` — the LSP tests, among them
  `formatting_replaces_the_document_with_the_ridl_fmt_rendering` and its
  `tabs()` and `four_spaces()` options.
- `crates/ridl/tests/baseline_desk.rs` — two tests pin the text of
  `crates/ridl/tests/baseline-corpus/cluster.ridl`
  (`check_reports_ordinal_drift_against_the_committed_baseline`,
  `inline_shape_removal_spans_the_service_name`).
- `crates/ridl/tests/book_examples.rs` — `fenced_blocks` (extracts the book
  fences with mdBook's options, `MDBOOK_OPTIONS`) and `book_examples_compile`
  (compiles every verified fence).
- `justfile` — the `wasm-check` recipe's crate list.

## 3. Tasks

Work the tasks in order; each is one commit and one review. For every task:
write the tests first, run them and watch them fail for the expected reason,
then write the code until they pass, then run the acceptance check. Every task
also keeps the four invariants of §4 green and updates the module documentation
of `crates/ridl-fmt/src/lib.rs` for the rules it adds, so the crate doc always
states the rules in force.

The order differs from the order of the note in one place: the breaking loop
(Task 3) comes before the CLI and LSP plumbing (Task 5), because a test that the
CLI reads the width needs a construct that breaks.

Acceptance commands used below: `cargo test -p ridl-fmt`,
`cargo test -p
ridl-cli --test facade`, `cargo test -p ridl-lsp`, and the gates
of §5.

### Task 1 — `FormatOptions`, no behaviour change

- Build: `pub struct FormatOptions { pub max_line_length: Option<usize> }` with
  `Default` giving `Some(100)` (note §6.6). Change the signature to
  `format(text, profile, &FormatOptions)`. Every caller passes
  `&FormatOptions::default()`: `run_fmt` in `crates/ridl/src/main.rs`, the
  `formatting` handler in `crates/ridl-lsp/src/server.rs`, and the tests. The
  options are threaded down to the layout functions but read by nothing yet. Add
  `-p ridl-fmt` to the `--no-default-features` crate list of `wasm-check` in
  `justfile` (note §6.6).
- Files: `crates/ridl-fmt/src/lib.rs`, `crates/ridl-fmt/tests/*.rs`,
  `crates/ridl/src/main.rs`, `crates/ridl-lsp/src/server.rs`, `justfile`.
- Tests first: a unit test that `FormatOptions::default().max_line_length` is
  `Some(100)`. Every existing test passes unchanged.
- Acceptance: `cargo test --workspace --locked` and `just wasm-check` pass; no
  golden file changes (`git diff --stat crates/ridl-fmt/test_data` is empty).

### Task 2 — the invariant harness over the three profiles

- Build: the checks of note §9 and §4 below, before any layout changes, so every
  later task runs them. In `crates/ridl-fmt/tests/properties.rs`: run
  `formatting_is_idempotent_over_the_ok_corpus` over the `.typl`, `.ridl` and
  `.rsdl` files of `crates/ridl-syntax/test_data/parser/ok`, each under the
  profile of its extension, at widths 100, 60 and 40; add the structure
  comparison of §4 (invariant 4) beside `content_tokens`; add a test that every
  file of the `err` corpus of the three profiles yields
  `FormatOutcome::ParseErrors`. In `crates/ridl-fmt/tests/corpus.rs`: pick the
  profile from the extension instead of listing `.typl` only.
- Tests first: extend
  `content_tokens_detect_a_dropped_comment_and_a_mutated_literal` so that it
  also deletes one member and checks that the structure comparison fails. That
  mutation test fails until the comparison exists. The corpus tests pass at once
  today, because the fallback arm is verbatim.
- Acceptance: `cargo test -p ridl-fmt`.

### Task 3 — the breaking loop and the typl tuple

- Build: the algorithm of note §6.2 (render on one line; while a line exceeds
  the width, break the last breakable construct on it and measure again) and the
  first breakable construct of §6.3, the tuple type of a typl field and of an
  array or map element (§6.5), with the commas of D-13. The three qualifications
  of §6.1: a trailing comment does not count, an unbreakable token stays, only
  line breaks read the width. `max_line_length: None` disables breaking.
- Files: `crates/ridl-fmt/src/lib.rs`, and `.typl` goldens only if a test needs
  a new pair.
- Tests first (the "Width" paragraph of note §11, for the tuple): one-line
  renderings of 99, 100 and 101 characters at the default width; the same three
  at a width of 60; a tuple inside an array type (the §6.3 example); a trailing
  comment past the width that causes no break; an unbreakable 120-character
  string literal; `None` leaving a 200-column line alone; a width of 1. Each
  test formats its output a second time and asserts equality.
- Acceptance: `cargo test -p ridl-fmt`; the existing `.typl` goldens do not
  change (note §6.5).

### Task 4 — the `.editorconfig` reader

- Build: a default cargo feature `editorconfig` in `crates/ridl-fmt/Cargo.toml`,
  an optional dependency on `ec4rs` `"1.2"` (release 1.2.0; `rust-version` 1.56,
  below the workspace's edition-2024 floor of 1.85 and the pinned toolchain
  1.98.1) with its default features off (no `language-tags`), added to
  `[workspace.dependencies]` in the root `Cargo.toml`. Before you commit the
  dependency, stop and ask the maintainer how its Apache-2.0 licence text is
  carried (§7); and a feature-gated module with one function,
  `FormatOptions::for_path(path: &Path) -> FormatOptions` (note §6.6, "What is
  honoured"). Only `max_line_length` is read: an integer is the width, `off` is
  `None`, absent, `unset` or any other value is `Some(100)`. `format` itself
  reads no file.
- Files: `Cargo.toml`, `Cargo.lock`, `crates/ridl-fmt/Cargo.toml`, a new module
  under `crates/ridl-fmt/src/`.
- Tests first, each in a temporary directory: `root = true` with
  `[*.ridl] max_line_length = 60` gives 60 for a `.ridl` file and 100 for a
  `.typl` file; `[*.{typl,ridl,rsdl}]` matches an `.rsdl` file; `off` gives
  `None`; `unset` and `abc` give 100; a nested directory's `.editorconfig` with
  `root = true` hides the outer file's value; a later section overrides an
  earlier one; no `.editorconfig` gives 100.
- Acceptance: `cargo test -p ridl-fmt`; `just wasm-check` (the crate still
  builds for wasm32 with `--no-default-features`); `just lint`.

### Task 5 — the CLI and the LSP pass the resolved width

- Build: `run_fmt` resolves `FormatOptions::for_path` per file, since two files
  of one run may sit under different `.editorconfig` files. The `formatting`
  handler resolves it from the document's path (`convert::uri_to_path`) and
  keeps ignoring the client's `FormattingOptions` (D-12).
- Files: `crates/ridl/src/main.rs`, `crates/ridl-lsp/src/server.rs`.
- Tests first: in `crates/ridl/tests/facade.rs`, the `.editorconfig` override
  tests of note §11, using a typl tuple field (the only construct that breaks so
  far) for the line that breaks at 60 and not at 100; in
  `crates/ridl-lsp/tests/server.rs`, the LSP test of note §11: a document in a
  temporary directory whose `.editorconfig` holds `root = true` and
  `[*.typl] max_line_length = 60`, `indent_size = 4`, `indent_style = tab`, with
  an 80-column tuple field line, receives the tuple broken at 60 and two-space
  indentation. It fails before the handler reads the file, because the handler
  then formats at 100. Do not add a test of the client's `FormattingOptions`
  alone: `formatting_replaces_the_document_with_the_ridl_fmt_rendering` already
  formats under `tabs()` and `four_spaces()` and asserts two-space indentation,
  so such a test would pass on its first run.
- Acceptance: `cargo test -p ridl-cli --test facade`, `cargo test -p ridl-lsp`.

### Task 6 — `interface` members, without attribute blocks

- Build: `InterfaceDef` through `format_block_def` with its modifiers, and the
  member renderings of note §3.1 and §3.2: `signal`, `event`, `fixed`,
  `command`, `query`, `reserved`; the payload type, the stream type, the
  parameter list, the four return shapes with `T | E` spaced (D-8), the init
  value, the four timing spellings. Breaking (§6.3): the parameter list and the
  tuple return, with commas (D-13). A member that carries an attribute block
  stays on the verbatim path until Task 7. Render every slot the parser accepts
  (note §3.1 table, `lenient_overapprox.ridl`), not only the slots of
  `family.ungram`: a stream payload and an init value on `signal`, `event` and
  `fixed`, a timing on `fixed`, a return on `command`.
- Stream type: add `StreamType` to `is_field_type` and a tight `StreamType` arm
  to `format_field_type` (note §3.1, "The stream type in a payload position").
  Without it `field_type()` returns an empty string for a stream payload, and
  today it already drops a stream in a `.ridl` struct field or array element.
- Block routing: when `InterfaceDef` goes through `format_block_def`, add it to
  the brace-block kinds for which `is_single_line_element` returns `false`
  (`StructDef`, `EnumDef`, `UnionDef`, and an `EnumSetDef` that has a `{`
  today). Otherwise `format_element` sees the between-member comments of the
  body as direct comment children (`has_direct_comment`) and emits the whole
  interface verbatim. The same applies to a `ServiceDef` with a brace body
  (Task 8) and to `SystemDef`, `ComponentDef`, `DistributionDef`,
  `DeploymentDef` and `MachineDef` (Tasks 9 and 10): each task adds its kinds,
  and adds a test of a body with a comment between two members.
- Files: `crates/ridl-fmt/src/lib.rs`; new `.ridl` pairs in
  `crates/ridl-fmt/test_data/input` and `formatted` (the §3.1 interface in the
  aligned style, without its `require` block).
- Tests first: one unit test per rule (note §11, "Unit tests"); the §6.2 worked
  example's parameter-list and tuple-return breaks at width 60; the 99/100/101
  boundary for both constructs; a comment inside a parameter list emits the list
  verbatim (note §5); `internal interface Hidden {}`; an interface with a
  comment between two members, which is laid out and not emitted verbatim; each
  lenient slot of `lenient_overapprox.ridl` without an attribute block; a
  `.ridl` struct with a field `a: <T>` and a field `b: [<T>; 1..2]`, which keeps
  both streams.
- Acceptance: `cargo test -p ridl-fmt`; `cargo test -p ridl-lsp` (the `.ridl`
  case of `formatting_replaces_the_document_with_the_ridl_fmt_rendering` is
  already canonical and must not change).

### Task 7 — attribute blocks and expressions

- Build: the inline form with padding (D-3), the block form for a block with a
  predicate or a block that does not fit (D-2), timing before the attribute
  block whatever the source order (D-4), one space after `require` and `ensure`
  (D-5), the canonical expression spacing (D-6), flags, assignments, dotted
  keys, value lists and nested value lists, with the value list breakable (§6.3,
  D-13). The two new comment positions of note §5 for a block-form attribute
  block. Add the D-4 normalisation of the `Timing`/`AttrBlock` pair to the two
  token comparisons of Task 2 (note §9, invariants 3 and 4). Update the crate
  doc's "whitespace and separators only" sentence with the D-4 exception.
- Files: `crates/ridl-fmt/src/lib.rs`, `crates/ridl-fmt/tests/properties.rs`,
  goldens.
- Tests first: the annotation pair in both source orders; an expression with
  every operator class (`require position!=GearPosition.PARK||currentSpeed==0.0`
  and the §3.2 rendering); parentheses kept as written; the §6.2 worked example
  at width 60 in full; a comment between two attributes of a block-form block; a
  comment inside one attribute; the 99/100/101 boundary for the inline block and
  for a value list; a mutation test that the D-4 normalisation still detects a
  dropped comment.
- Acceptance: `cargo test -p ridl-fmt`.

### Task 8 — `service`

- Build: the named form and the inline form of note §3.3 (D-7): the shape list
  on one line when it fits, trailing comma removed; otherwise broken after the
  colon, one shape per line, the required comma after every shape but the last.
  The inline form's body reuses Task 6 and Task 7.
- Files: `crates/ridl-fmt/src/lib.rs`, goldens.
- Tests first: both forms, with and without a trailing comma; the §3.3 before
  and after; a list that breaks at width 40 and is a fixed point; a trailing
  comment after a named form's list stays on the line.
- Acceptance: `cargo test -p ridl-fmt`; add a `crates/ridl-fmt/tests/ridl.rs`
  sibling of `rsdl.rs` over the ridl reference's Appendix A fences (idempotence
  and tokens, note §11 "Reference examples").

### Task 9 — `system`, `distribution`, `component`, and the `machine` body

- Build: note §4.1, §4.2 and §4.4: the brace block with one member per line
  always (D-1), the member line with its inline attribute block, `offers` and
  `requires` with one space and no padding, the header attribute block between
  the name and `{`, and the block form with the brace on the closer's line when
  the header does not fit (§6.4). Reuse the attribute block of Task 7. Extend
  `block_header_prefix` to render the header attribute block, and keep its
  verbatim path for a comment that is a direct token of the header (note §5).
  Add `SystemDef`, `ComponentDef` and `DistributionDef` to the kinds for which
  `is_single_line_element` returns `false` (Task 6).
- Files: `crates/ridl-fmt/src/lib.rs`; `crates/ridl-fmt/tests/rsdl.rs`;
  `crates/ridl/tests/facade.rs`; `crates/ridl-lsp/tests/server.rs`; `.rsdl`
  goldens (the issue's `component`, `rsdl_attribute_positions.rsdl`).
- Tests that this task breaks, and how each changes (note §8 D-1 and §11):
  - `crates/ridl-fmt/tests/rsdl.rs`:
    `an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written`
    is rewritten to the §4 renderings; `tokens()` filters out
    `SyntaxKind::Comma` tokens as well as trivia, because D-1 removes the
    separator commas of the reference fences that
    `the_reference_rsdl_examples_format_idempotently_and_keep_every_token`
    reads; the module documentation, which says each rsdl declaration is emitted
    as written, is rewritten.
  - `crates/ridl/tests/facade.rs`, `fmt_formats_an_rsdl_file`: the expected
    output becomes
    `"package veh.topology\n\ncomponent Cruise {}\n\nsystem Vehicle {\n  Cruise\n}\n"`,
    and its doc comment no longer says each declaration is kept as written.
    `fmt_check_walks_into_rsdl_files` does not change (it still exits 1).
  - `crates/ridl-lsp/tests/server.rs`,
    `formatting_replaces_the_document_with_the_ridl_fmt_rendering`, the `.rsdl`
    case (request id 18): the expected `new_text` becomes
    `"package solo\n\ncomponent Door {\n  offers solo.door\n}\n"`. Its `.ridl`
    case is already canonical and does not change.
- Tests first: each before/after of §4.1, §4.2 and §4.4; the §4.4 header at
  width 143 (block form) and at width 144 (one line: the joined header is 144
  columns, measured); an empty body `{}`; a dotted key; a comment on the
  opening-brace line; a comment between two members of a `component`; a comment
  between the name and the header attribute block (header verbatim) and one
  inside it (attribute-block rules).
- Acceptance: `cargo test -p ridl-fmt`, `cargo test -p ridl-cli --test facade`,
  `cargo test -p ridl-lsp`; the `.rsdl` files of the parser `ok` corpus pass the
  Task 2 checks at the three widths.

### Task 10 — `deployment` and nested `machine` blocks

- Build: note §4.3: the header `deployment Name for System [ attrs ] {`, the
  `machine` blocks nested one level down through `format_block_def`, blank lines
  between machines preserved where the source had one (D-11), the comment
  between two machines placed as a between-member comment (note §5).
- Files: `crates/ridl-fmt/src/lib.rs`, goldens (Appendix A of the rsdl
  reference).
- Tests first: the §4.3 before and after; a nested `machine` with and without
  blank lines; a comment between two machines; a comment in a deployment header.
- Acceptance: `cargo test -p ridl-fmt`; none of the seven declarations reaches
  the fallback arm of `format_element` any more, and the comment above that arm
  no longer names them.

### Task 11 — the repository's `.editorconfig`

- Build: the `[*.{typl,ridl,rsdl}]` section of note §6.6 in the root
  `.editorconfig`. No test: the default width is already 100, so the formatter's
  output does not change; the section is for editors.
- Acceptance: `just check`; `cargo test --workspace --locked` passes unchanged.

### Task 12 — the reformatting sweep (D-10 (b)) and the fixed-point tests

- Build: reformat, with `cargo run -p ridl-cli --bin ridl -- fmt <path>`, the
  fixtures that are meant to be canonical
  (`crates/ridl-fmt/test_data/formatted`, already canonical by its own test),
  `examples/` (`examples/cabin/cabin.ridl` and any other `.typl`, `.ridl` or
  `.rsdl` file there), `crates/ridl/tests/baseline-corpus/cluster.ridl`, and
  every verified `ridl`, `typl` and `rsdl` fence written directly in a
  `docs/book/` file. Do not touch the parser corpus under
  `crates/ridl-syntax/test_data/` (its layout is test input), the `ignore`
  fences, or the files the book pulls in with `{{#include}}` (they are the
  references, out of scope). To reformat a fence, copy its text to a temporary
  file with the matching extension, run `ridl fmt` on it, and paste the result
  back.
- Tests the sweep breaks, and how each changes (note §11, "Round trips"):
  reformatting `cluster.ridl` tightens its `name : Type` colons and inserts a
  blank line between its three `type` declarations, so every later line moves
  down by two. In `crates/ridl/tests/baseline_desk.rs`,
  `check_reports_ordinal_drift_against_the_committed_baseline` expects the
  aligned source lines of `tyrePressure`, `legacyWheelPhase`, `doorOpened` and
  `doorClosed` (for example `"event tyrePressure : DoorState @[100ms..1s]"`):
  each becomes the tight form (`"event tyrePressure: DoorState @[100ms..1s]"`).
  `inline_shape_removal_spans_the_service_name` expects `cluster.ridl:46:9`: it
  becomes the `service` line's new number (48 with today's blank-line rule; read
  it with `grep -n '^service' cluster.ridl` after the sweep), column 9. The
  committed `.ridl/baseline/corpus.baseline.ir.json` holds no source position
  and does not change.
- Book prose the sweep makes false, corrected in the same commit (note §8 D-10):
  - `docs/book/getting-started.md`: the inline code at line 136
    (`type Speed : km/h [0.0..250.0 step 0.5]`), line 164
    (`signal currentSpeed : Speed @10ms`), lines 204-205
    (`fixed doorCount : integer [1..8]`, `type DoorCount : integer [1..8]`) and
    lines 479-480 (`hasCruise : Enabled`, `hasCruise : boolean`) take the tight
    colon; the `text` fence at lines 182-188 is regenerated by running
    `ridl check` on the reformatted `types.ridl` with the bounds swapped (the
    location becomes `./veh/common/types.ridl:4:18` and the quoted line
    `type Speed: km/h [250.0..0.0 step 0.5]`); the paragraph "`ridl fmt` has its
    own canonical layout" at lines 818-820, which says the listings use the
    aligned layout, is rewritten to say they are in the formatter's layout.
  - `docs/book/cli-reference.md` lines 871-872 (in the paragraph at lines
    868-872): the sentences saying the formatter has no layout rules for the
    rsdl declarations and keeps each one, and a ridl `interface` or `service`,
    as written are replaced by a short statement of the new rules, the
    100-column default and the `max_line_length` key of `.editorconfig`.
  - Leave `docs/book/getting-started.md` line 214 (it quotes the typl
    reference's own `frame : bytes [8]`, which is out of scope) and the
    diagnostics of `cli-reference.md`, whose sources are not in any verified
    fence.
  - Before committing, grep `docs/book/` again for inline code with a space
    before a colon and for `ridl fmt`, and check every hit against the
    reformatted fences.
- Tests first: a fixed-point test over `examples/` and the book fences (note
  §11, "Round trips") — every verified fence, extracted with `fenced_blocks` in
  `crates/ridl/tests/book_examples.rs`, formats to itself; the same for
  `examples/cabin/cabin.ridl` and `cluster.ridl`. They fail before the sweep and
  pass after it.
- Acceptance: `cargo test -p ridl-cli --test book_examples` (the book harness
  still compiles every fence with its `allow=` markers), `just demo`,
  `cargo test -p ridl-cli --test baseline_desk --test baseline_gate`,
  `cargo test --workspace --locked`, `just book-check`, `just link-check`.

### Task 13 — documentation

- Build: the `crates/ridl-fmt` entry of
  `docs/technotes/walking-skeleton-architecture.md` (the formatter now lays out
  every declaration of the three profiles and reads the width; its lines 189-192
  also omit `.rsdl` from the files `ridl fmt` reads); `crates/ridl/src/main.rs`
  line 6, which still says `ridl fmt` formats `.typl` files only; the
  `description` in `crates/ridl-fmt/Cargo.toml` line 7 ("the typl surface"); the
  first line of the `crates/ridl-fmt/src/lib.rs` module documentation, which
  says the formatter is "for the typl surface"; the comment above the fallback
  arm of `format_element` (`crates/ridl-fmt/src/lib.rs` lines 331-333), which
  says the five rsdl declarations reach that arm although a `machine` reaches it
  only inside its `deployment` — Task 10 removes the declarations from the arm,
  so check that the comment left there no longer names them; a final read of the
  `crates/ridl-fmt/src/lib.rs` module documentation against the note.
- Acceptance: `just link-check`, `just doc-path-check`, `just check`,
  `cargo doc -p ridl-fmt --no-deps`.

## 4. Invariants every task keeps

Note §9 states them; Task 2 builds the checks, and every later task runs them
over its new inputs as well as over the corpus.

1. **Total.** An input with a parse error is returned as
   `FormatOutcome::ParseErrors` and untouched. Check: the
   `broken_input_returns_parse_errors_untouched` unit test pattern in
   `crates/ridl-fmt/src/lib.rs`, plus the `err` corpus test of Task 2. Each task
   adds one broken input of the declaration it formats.
2. **Idempotent.** `format(format(x)) == format(x)`, including lines the width
   broke. Check: every new unit test formats its output a second time;
   `formatting_is_idempotent_over_the_ok_corpus` at widths 100, 60 and 40.
3. **No comment lost.** Every comment of the input is in the output, in order,
   with the same text. Check: `content_tokens` in
   `crates/ridl-fmt/tests/properties.rs`, which compares every token that is not
   whitespace or a comma, comments included; from Task 7, after the D-4
   normalisation.
4. **The parse tree is unchanged apart from trivia.** Check: the structure
   comparison of Task 2 — parse the input and the output, walk both with
   `descendants_with_tokens`, and compare the sequences of node kinds (on entry
   and exit) and of token kinds and texts, skipping whitespace, comments and
   `Comma` tokens; from Task 7, after the D-4 normalisation. The
   `source_order_is_never_changed` unit test gains an interface and an rsdl
   variant.

A mutation test proves each check can fail: delete one comment, one member and
change one literal in a formatted output, and assert the comparison reports it
(`content_tokens_detect_a_dropped_comment_and_a_mutated_literal` is the model).

## 5. Repository rules

- Run `./bootstrap` after `git clone` or `git worktree add`.
- Work on a branch made from `main`, for example `feat/387-fmt-options`. Never
  commit to `main` and never push to it; every change goes through a pull
  request.
- Commit messages follow Conventional Commits, linted by `git std` against
  `.git-std.toml`, whose `types` and `scopes` are explicit lists. The scopes
  this plan needs: `ridl-fmt`, `ridl-lsp`, `ridl` (the CLI crate), `docs`,
  `deps` (the `ec4rs` dependency, if committed on its own), `repo`
  (`.editorconfig`, `justfile`). Example:
  `feat(ridl-fmt): lay out interface
  members`.
- Comments, commit messages, documentation and pull request descriptions are in
  plain, literal English, with no idioms or figures of speech. Technical terms
  stay as they are.
- Before every push, run and pass: `just fmt-check`, `just lint`, `just check`,
  `just test`, `just wasm-check`, `just book-check`, `just link-check`,
  `just doc-path-check`. Before opening a pull request, run `just verify`. Never
  use `--no-verify`; if a hook fails, fix the cause.
- `cargo fmt --all` repairs Rust formatting; `just fmt` repairs Markdown, JSON,
  YAML and TOML formatting (prim).
- One pull request per task, or per small group of consecutive tasks. Write
  `Part of #387` in each pull request body. Write `Closes #387` only in the pull
  request that completes the issue (§6). Never write a closing keyword (`close`,
  `closes`, `fix`, `fixes`, `resolve`, `resolves`, and their other forms) next
  to any other issue number, not even in a negation such as "does not close #N":
  GitHub closes the issue on merge.
- Do not invent attribution lines. Use the commit trailers the repository's
  recent history uses (`git log -20 --format='%(trailers)'`), or none.
- Do not merge your own pull request; the maintainer merges.

## 6. Done criteria for #387

- Every declaration of note §3 and §4 is formatted as the note states, and none
  reaches the fallback arm of `format_element`.
- The width rules of note §6 hold for every breakable construct of §6.3, at the
  default width and at a width set through `FormatOptions`.
- `ridl fmt` and the LSP `formatting` handler read `max_line_length` from
  `.editorconfig`; the repository's `.editorconfig` has the section of note
  §6.6; `ridl-fmt` builds for wasm32 with `--no-default-features` in
  `just wasm-check`.
- The D-10 (b) sweep is done and held by the fixed-point tests of Task 12.
- The documentation of Task 13 is updated.
- `just verify` passes on the last pull request, which carries `Closes #387`.
- Working memory: when the work lands, the design note and this plan move from
  `docs/wip/` to `docs/archive/`, their entries in `docs/wip/README.md` move to
  `docs/archive/README.md`, and their durable content is gardened into `docs/`
  (note, "Trace links", Disposition). This is a maintainer-run step; do it only
  when the maintainer asks.

## 7. Stop and ask the maintainer

Stop, describe the case with a minimal input, and ask before going further when:

- the note does not settle a case — an input whose rendering or break position
  the note does not determine;
- a grammar form appears that the note does not list (note §1 lists every form
  it covers);
- the implementation would need a change to the parse tree, the parser, the
  grammar, or a diagnostic;
- a check of §4 fails on an input and the fix is not in the formatter;
- the note and the code disagree on a fact (a function name, a file path, a test
  name);
- at Task 4, before committing the `ec4rs` dependency: ask how its Apache-2.0
  licence text is carried with the distributed binary. The maintainer decides;
  the repository had no notices file or notices tooling when this plan was
  written. The maintainer decided on 2026-10-01 to keep MIT and ship
  `THIRD-PARTY-NOTICES.txt`; that decision is recorded in note §6.6.
