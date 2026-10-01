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
  the verbatim fallback the seven declarations reach today), `format_block_def`,
  `block_header_prefix`, `split_brace_line_comment`, `format_field_type`,
  `format_constraint`, `format_reserved_entry`, `tight_text`,
  `is_single_line_element`, `has_direct_comment`. The module documentation at
  the top states the rules in force.
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
- `crates/ridl-lsp/tests/server.rs` — the LSP tests.
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
  an optional dependency on `ec4rs` 1.2 with its default features off (no
  `language-tags`), added to `[workspace.dependencies]` in the root
  `Cargo.toml`, and a feature-gated module with one function,
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
  `crates/ridl-lsp/tests/server.rs`, a formatting request with
  `FormattingOptions { tabSize: 4, insertSpaces: false }` that receives
  two-space indentation.
- Acceptance: `cargo test -p ridl-cli --test facade`, `cargo test -p ridl-lsp`.

### Task 6 — `interface` members, without attribute blocks

- Build: `InterfaceDef` through `format_block_def` with its modifiers, and the
  member renderings of note §3.1 and §3.2: `signal`, `event`, `fixed`,
  `command`, `query`, `reserved`; the payload type, the stream type, the
  parameter list, the four return shapes with `T | E` spaced (D-8), the init
  value, the four timing spellings. Breaking (§6.3): the parameter list and the
  tuple return, with commas (D-13). A member that carries an attribute block
  stays on the verbatim path until Task 7.
- Files: `crates/ridl-fmt/src/lib.rs`; new `.ridl` pairs in
  `crates/ridl-fmt/test_data/input` and `formatted` (the §3.1 interface in the
  aligned style, without its `require` block).
- Tests first: one unit test per rule (note §11, "Unit tests"); the §6.2 worked
  example's parameter-list and tuple-return breaks at width 60; the 99/100/101
  boundary for both constructs; a comment inside a parameter list emits the list
  verbatim (note §5); `internal interface Hidden {}`.
- Acceptance: `cargo test -p ridl-fmt`.

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
  the header does not fit (§6.4). Reuse the attribute block of Task 7.
- Files: `crates/ridl-fmt/src/lib.rs`, `crates/ridl-fmt/tests/rsdl.rs` (rewrite
  `an_rsdl_file_takes_the_file_layout_and_keeps_its_declarations_as_written` to
  the §4 renderings), `.rsdl` goldens (the issue's `component`,
  `rsdl_attribute_positions.rsdl`).
- Tests first: each before/after of §4.1, §4.2 and §4.4; the §4.4 header at
  width 100 (block form) and at width 140 (one line); an empty body `{}`; a
  dotted key; a comment on the opening-brace line.
- Acceptance: `cargo test -p ridl-fmt`; the `.rsdl` files of the parser `ok`
  corpus pass the Task 2 checks at the three widths.

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
- Tests first: a fixed-point test over `examples/` and the book fences (note
  §11, "Round trips") — every verified fence, extracted with `fenced_blocks` in
  `crates/ridl/tests/book_examples.rs`, formats to itself; the same for
  `examples/cabin/cabin.ridl` and `cluster.ridl`. They fail before the sweep and
  pass after it.
- Acceptance: `cargo test -p ridl-cli --test book_examples` (the book harness
  still compiles every fence with its `allow=` markers), `just demo`,
  `cargo test -p ridl-cli --test baseline_desk --test baseline_gate`,
  `just book-check`.

### Task 13 — documentation

- Build: the `crates/ridl-fmt` entry of
  `docs/technotes/walking-skeleton-architecture.md` (the formatter now lays out
  every declaration of the three profiles and reads the width);
  `crates/ridl/src/main.rs` line 6, which still says `ridl fmt` formats `.typl`
  files only; the `description` in `crates/ridl-fmt/Cargo.toml` ("the typl
  surface"); a final read of the `crates/ridl-fmt/src/lib.rs` module
  documentation against the note.
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
- the `ec4rs` dependency needs a licence notice: the note says its licence text
  ships with the binary's notices, and the repository has no notices file today.
