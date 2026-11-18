# `match` patterns as I-Regexp — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** a typl `match` pattern is an I-Regexp (RFC 9485) minus six spellings,
matched against the whole value, with several `match` clauses per string type,
one checker verdict, and one engine text per output (driftsys/ridl#597).

**Architecture:** a pure recogniser in `ridl-sem` replaces `regress`; one
normalisation function in `ridl-ir` produces the text every backend wraps; the
IR and the codegen model gain `repeated Match matches` and keep writing the
deprecated `pattern` and `pattern_const` fields for the first clause.

**Tech Stack:** Rust (rowan, prost, `regex` 1.13.1), TypeScript under Deno.

**Spec:**
[`2026-11-18-i-regexp-match-patterns-design.md`](2026-11-18-i-regexp-match-patterns-design.md)
(cited below as "the note"; every decision is in its §12 and is not reopened
here).

## Rules for the implementer

Each task runs in a `codex exec` sandbox, alone, with no memory of the other
tasks. The sandbox cannot commit, cannot bind a TCP port and has no network.

- Do not run `git commit`. Leave the change in the working tree; the last step
  of each task gives the commit message that the driver uses.
- Do not edit a file under `freeze/` by hand. When a step says so, run
  `cargo xtask freeze update` and report which files changed.
- Do not add a dependency that is not already in `Cargo.lock`. Adding an
  existing workspace dependency (`regex`) to a crate's `[dev-dependencies]` is
  allowed.
- Run only the focused commands the task names, not `just build`.
- Insta snapshots: when a step says to refresh them, run the named test with
  `INSTA_UPDATE=always`, then read the diff of every changed `.snap` file and
  confirm that only pattern text, `matches` entries or catalog hashes changed.
- Text written into `crates/`, `docs/book/`, `docs/design/` or
  `docs/specification/` names no story, stage, lane or epic
  (`just
  story-id-check`). Prose is plain and literal.
- A test is written first and must fail by assertion. Where a new function would
  make the test fail by a compile error, first add the function with a body that
  returns a wrong value (`String::new()`, `Ok(())`), then write the test, then
  see it fail.

## Global Constraints

- Language: the note §3.1 to §3.6. Refused beyond the RFC 9485 ABNF: `^` and `$`
  outside a class; `&&`, `~~` inside a class; an unescaped `-` directly followed
  by an unescaped `-` inside a class; `\-` outside a class; `\p{..}` and
  `\P{..}`; `[^]`; a range whose end is before its start; a quantifier whose
  maximum is below its minimum.
- One diagnostic: TYPL-106 for every pattern outside the language, inline or in
  a regex `const`. No new diagnostic code. TYPL-220 stays for an accepted
  pattern that `regex` refuses.
- Engine texts (note §7), with `P` the normalised pattern: Rust and the checker
  `\A(?:P)\z` with `is_match`; TypeScript `new RegExp("^(?:P)$", "u")` with
  `test`.
- Deprecated fields (note §6): `ridl.codegen.v1.Constraint.pattern` is
  `^(?:P)$`; `ridl.ir.v2.Constraint.pattern` is `/^(?:S)$/` with `S` the text
  between the delimiters of the first clause's source literal, `\/` unchanged.
  Both hold the first clause only.
- Input written by 0.20 (empty `matches`, a present `pattern` or
  `pattern_const`) is one clause with the old meaning: a search with that text.
- `ridl-rt` is not changed (`Rule::Pattern` keeps its shape).
- `Constant.regex_body` and `ConstDef.regex` keep their shape and text.
- `regress` leaves the workspace in Task 7.

## Review Focus

1. A literal whose body holds `\\` before `/` or before `\/` (`/a\\\/b/`): the
   delimiter escape is a `\` that an even number of backslashes precedes. Task 1
   pins it.
2. A non-ASCII or astral character next to `.` or a negated class: with the `u`
   flag the TypeScript output counts it as one character, as Rust does. Task 6
   puts `"\u{1F600}"` in the agreement table.
3. An IR snapshot or codegen request written by 0.20: the backends keep the old
   search, and `ridl diff` reads it as one clause. Tasks 3, 4 and 6 test it.
4. A constant named in a second or later clause: the reference walk, the catalog
   hash canonical form, the design lint and the `ridlc` dependency scan must see
   it. Task 3 tests the second clause for each.
5. The line terminators `\r`, U+2028 and U+2029 under `.`: the Rust and the
   TypeScript outputs must give the same verdict. Task 6 puts them in the
   agreement table.

## Task order and why

The order keeps `cargo test --workspace` green after every task.

- Tasks 1 and 2 add pure functions that nothing calls yet.
- Task 3 adds `matches` and moves every reader, with the meaning unchanged:
  every clause is still a search with today's text.
- Task 4 makes `ridl diff` compare clause sets. It is separate from Task 3
  because its tests are independent and the reviewer can reject it alone.
- Task 5 lets the source carry several clauses. Every reader already iterates
  `matches`, so no clause is accepted and then ignored.
- Task 6 switches the outputs to whole-value matching. Every pattern accepted
  before Task 7 is ECMA-262 that `regex` also compiles, and wrapping it in
  `\A(?:…)\z` or `^(?:…)$` changes only a pattern with no anchors, so Task 6
  migrates the two in-repository patterns whose tests rely on a search. This is
  the first breaking commit: it changes what generated code accepts for an
  unanchored pattern.
- Task 7 is the breaking switch of the language: the recogniser decides TYPL-106
  and every pattern literal that it would refuse is migrated in the same task.
- Task 8 is documentation only.

## Pull requests

- **PR 1 — Tasks 1 to 5, not breaking.** Adds the normaliser, the recogniser
  (unused), the `matches` field and several `match` clauses with today's
  dialect. The spec text for several clauses and for the new IR fields lands in
  Tasks 3 and 5, so the shipped docs describe what PR 1 builds. Frozen surface:
  `freeze/codegen-protocol.txt` gains lines and loses none. PR 1 changes the
  catalog hash of every unit that reaches a type with a pattern (the canonical
  form now holds `matches`); PR 2 changes it again. Release PR 1 and PR 2 in the
  same minor release, so that each compatibility chain restarts once (the note
  §9).
- **PR 2 — Tasks 6 to 8, breaking (`!`).** Whole-value matching, the I-Regexp
  check, the migration, `regress` removed, and the records of the note §13.
  Frozen surfaces: `freeze/generated/veh-cluster/ridl.std.rs`,
  `freeze/generated/veh-cluster/veh.common.rs` and
  `freeze/generated/veh-cluster/veh.cluster.rs` change (the engine text in Task
  6, the doc-comment pattern lines and the `*_PATTERN` const values in Task 7).
  No other dump is expected to change; `freeze/generated/cabin/` holds no
  pattern. The `generated-crate` row compares Rust APIs, not lines, so no
  baseline line is removed; if `cargo xtask freeze check` reports an API break
  (a changed `pub const` value), stop and report it: the driver follows "Approve
  a break" in `freeze/README.md`. After the merge, the driver comments on
  driftsys/ridlc-gen-kotlin#186 (the note §13, last item).

---

### Task 1: The normalised pattern and the engine texts in `ridl-ir`

**Files:**

- Create: `crates/ridl-ir/src/pattern.rs`
- Modify: `crates/ridl-ir/src/lib.rs:1166` (add `pub mod pattern;` in the sorted
  `pub mod` list, after `name`)
- Modify: `crates/ridl-ir/Cargo.toml:30` (`regex.workspace = true` under
  `[dev-dependencies]`)

**Interfaces:**

- Produces (all `pub` in `ridl_ir::pattern`):
  - `fn pattern_text(literal: &str) -> String` — removes one leading and one
    trailing `/` when both are present, then replaces each delimiter escape `\/`
    by `/`. Scan left to right: a `\` and the character after it form one pair;
    a pair `\/` becomes `/`, every other pair is copied unchanged.
  - `fn normalised_pattern(literal: &str) -> String` — `pattern_text`, then
    every `.` outside a class is written `[^\n\r]` (the six characters `[`, `^`,
    `\`, `n`, `\`, `r`, `]`). A class starts at an unescaped `[` and ends at the
    next unescaped `]`. An escape pair is copied unchanged inside and outside a
    class. Total: any input returns a string and never panics (an unclosed class
    runs to the end).
  - `fn rust_engine_text(normalised: &str) -> String` — `\A(?:{normalised})\z`.
  - `fn search_engine_text(normalised: &str) -> String` — `^(?:{normalised})$`.
    The TypeScript `RegExp` source and the deprecated model field.
  - `fn deprecated_ir_literal(literal: &str) -> String` — `/^(?:{S})$/`, `S` the
    literal without its two delimiters, `\/` kept.

- [ ] **Step 1: Write the failing tests** in a `#[cfg(test)] mod tests` of
      `pattern.rs` (stub bodies return `String::new()`):

```rust
assert_eq!(pattern_text(r"/a\/b/"), "a/b");
assert_eq!(pattern_text(r"/a\\\/b/"), r"a\\/b");
assert_eq!(pattern_text(r"/a\.b/"), r"a\.b");
assert_eq!(normalised_pattern(r"/a.b/"), r"a[^\n\r]b");
assert_eq!(normalised_pattern(r"/[.]/"), "[.]");
assert_eq!(normalised_pattern(r"/\./"), r"\.");
assert_eq!(normalised_pattern(r"/[\].]./"), r"[\].][^\n\r]");
assert_eq!(normalised_pattern(r"/(.|\n)*/"), r"([^\n\r]|\n)*");
assert_eq!(normalised_pattern(r"/a\/./"), r"a/[^\n\r]");
assert_eq!(normalised_pattern("/[/"), "[");          // total, no panic
assert_eq!(rust_engine_text("a|b"), r"\A(?:a|b)\z");
assert_eq!(search_engine_text("a|b"), "^(?:a|b)$");
assert_eq!(deprecated_ir_literal(r"/a\/b|c/"), r"/^(?:a\/b|c)$/");
```

Add `engine_texts_agree_under_the_regex_crate`: for the literals `/a.b/`,
`/[0-9]{2}/`, `/x|yz/` and the values `"a-b"`, `"a\rb"`, `"a\nb"`, `"ab"`,
`"12"`, `"123"`, `"x"`, `"xyz"`, `"yz"`, assert that
`Regex::new(&rust_engine_text(&n)).is_match(v)` equals
`Regex::new(&search_engine_text(&n)).is_match(v)`, and that `/a.b/` refuses
`"a\rb"` and `"a\nb"` and accepts `"a-b"`.

- [ ] **Step 2: Run** `cargo test -p ridl-ir pattern::` — expected: FAIL on the
      first `assert_eq!`.
- [ ] **Step 3: Implement** the five functions.
- [ ] **Step 4: Run** `cargo test -p ridl-ir pattern::` and
      `cargo clippy -p ridl-ir --all-targets -- -D warnings` — expected: PASS.

**Mutations the tests must catch:** drop the `.` rewrite (the `/a.b/` and `\r`
assertions fail); rewrite `.` inside a class too (`/[.]/` fails); treat `\\/` as
a delimiter escape (`/a\\\/b/` fails).

- [ ] **Step 5: Commit message**

```text
feat(ridl-ir): add the normalised pattern and the engine texts

One function turns a typl regex literal into the pattern every backend
wraps: the delimiters removed, the delimiter escape \/ replaced by /, and
every . outside a class written [^\n\r]. Three more functions give the Rust
engine text, the search text, and the deprecated IR literal.

Refs #597
```

### Task 2: The I-Regexp recogniser in `ridl-sem`

**Files:**

- Create: `crates/ridl-sem/src/pattern.rs`
- Modify: `crates/ridl-sem/src/lib.rs:17` (add `pub mod pattern;` after
  `pub mod lint;`, keeping the list sorted)

**Interfaces:**

- Consumes: nothing (the input is already the output of
  `ridl_ir::pattern::pattern_text`).
- Produces: `pub fn check(pattern: &str) -> Result<(), Refusal>` and
  `pub struct Refusal { pub construct: String, pub message: String }`.
  `construct` is the refused source text (for example `^`, `\d`, `(?=`,
  `[z-a]`); `message` is the full diagnostic text. Task 7 calls `check`.

The grammar is the ABNF of RFC 9485 §3 (a copy is at
`/private/tmp/claude-501/-Users-sebastientasson-Workspace-driftsys-ridl/1eaa2570-abc6-421e-bf5e-56c3e6528f14/scratchpad/rfc/rfc9485.txt`,
lines 135-200), parsed by recursive descent with no backtracking, plus the
refusals of the Global Constraints. Quantifier numbers are read as `u64` with
saturation; a large number is not refused here (the `regex` crate decides it,
TYPL-220). The first refusal found from the left is returned.

Messages (exact; the help names the replacement of the note §3.2 or §3.3):

| Construct                                 | `message`                                                                                                                                                                                                |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `^` outside a class                       | ``"`^` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `^` is `\^`"``                                                                                                |
| `$` outside a class                       | ``"`$` is not accepted: a pattern matches the whole value, so remove the anchor; a literal `$` is `[$]`"``                                                                                               |
| `\A`, `\z`, `\Z`                          | ``"`\A` is not accepted: a pattern matches the whole value, so remove the anchor"`` (the construct as written)                                                                                           |
| `\d`, `\D`                                | ``"`\d` is not accepted: write `[0-9]`"``, ``"`\D` is not accepted: write `[^0-9]`"``                                                                                                                    |
| `\w`, `\W`                                | ``"`\w` is not accepted: write `[0-9A-Za-z_]`"``, `\W` with `[^0-9A-Za-z_]`                                                                                                                              |
| `\s`, `\S`                                | ``"`\s` is not accepted: write `[ \t\n\r]`"``, `\S` with `[^ \t\n\r]`                                                                                                                                    |
| `\b`, `\B`                                | ``"`\b` is not accepted: a pattern has no word boundary"``                                                                                                                                               |
| `\u`, `\x`                                | ``"`\x` is not accepted: write the character itself, or a range such as `[ -~]`"``                                                                                                                       |
| `\p{..}`, `\P{..}`                        | ``"`\p{..}` is not accepted: write explicit ranges, such as `[a-zà-ÿ]`"``                                                                                                                                |
| `\1`..`\9`, `\k`                          | `"a backreference is not accepted: write the alternatives out"`                                                                                                                                          |
| `(?=` `(?!` `(?<=` `(?<!`                 | ``"lookaround is not accepted: write each condition as its own `match` clause"``                                                                                                                         |
| `(?:`, `(?<name>`, `(?P<`                 | ``"`(?:` is not accepted: write a plain group `(…)`"`` (the construct as written)                                                                                                                        |
| other `(?`                                | ``"inline flags are not accepted: write each case in a class, such as `[Aa]`"``                                                                                                                          |
| quantifier after a quantifier             | ``"`*?` is not accepted: a pattern has no lazy or possessive quantifier"`` (the two characters as written)                                                                                               |
| `{,n}`                                    | ``"`{,3}` is not accepted: a quantifier needs a minimum, such as `{0,3}`"``                                                                                                                              |
| `{n,m}` with m < n                        | ``"the quantifier `{3,2}` has a maximum below its minimum"``                                                                                                                                             |
| `[` inside a class, `[:`                  | ``"a nested class is not accepted: a literal `[` is `\[`"``; for `[:alpha:]` ``"a POSIX class is not accepted: write the range, such as `[a-z]`"``                                                       |
| `&&`, `~~` inside a class                 | ``"`&&` is not accepted in a class: write one `&`"`` (and `~`)                                                                                                                                           |
| `--` inside a class                       | ``"`--` is not accepted in a class: write one `-`"``                                                                                                                                                     |
| `\-` outside a class                      | ``"`\-` is accepted inside a class only: write a bare `-`"``                                                                                                                                             |
| `[^]`                                     | ``"`[^]` is not accepted: a literal `^` is `\^`, and any character is `(.\|\n\|\r)`"``                                                                                                                   |
| `[z-a]`                                   | ``"the range `z-a` ends before it starts"``                                                                                                                                                              |
| any other escape `\X`                     | ``"`\X` is not an I-Regexp escape"``                                                                                                                                                                     |
| anything else (unbalanced, lone `{`, ...) | `"invalid regular expression syntax: {what}"` with `{what}` one of `unclosed group`, `unmatched )`, `unclosed class`, `a quantifier with nothing to repeat`, `unescaped {`, `unescaped }`, `unescaped ]` |

- [ ] **Step 1: Write the failing tests** (stub `check` returns `Ok(())`), as
      two tables in `pattern.rs`:
  - `ACCEPTED`: `a`, `` (empty), `ab|c`, `(a)`, `(a|)`, `a*`, `a+`, `a?`,
    `a{2}`, `a{2,}`, `a{2,3}`, `.`, `[a-z0-9_]`, `[^@]`, `[-a]`, `[a-]`,
    `[\--z]`, `[\]]`, `[$]`, `\^`, `\(`, `\)`, `\*`, `\+`, `\.`, `\?`, `\[`,
    `\\`, `\]`, `\{`, `\|`, `\}`, `\n`, `\r`, `\t`, `[\-]`, `é+`, `[à-ÿ]`,
    `\u{1F600}` (the character itself), every pattern of the "After" column of
    the `ridl.std` table in Task 7, and `(([a-z]{1,64}){1,64}){1,64}`.
  - `REFUSED`: `(pattern, construct)` pairs, one per row of the message table,
    including `^a`, `a$`, `\d`, `\w`, `\s`, `\D`, `\b`, `\A`, `\z`, `\x41`, `A`,
    `\p{L}`, `\P{L}`, `(a)\1`, `\k<w>`, `(?=a)`, `(?!a)`, `(?<=a)b`, `(?<!a)b`,
    `(?:a)`, `(?<w>a)`, `(?i)ab`, `a*?`, `a++`, `a{,3}`, `a{3,2}`, `[[a]]`,
    `[[:alpha:]]`, `[a&&b]`, `[a~~b]`, `[--]`, `[^--]`, `a\-b`, `[^]`, `[z-a]`,
    `\e`, `(`, `)`, `[`, `{`, `a{`, `}`, `]`, `*`. Each asserts `Err`, the
    `construct`, and for `^`, `$`, `\d`, `&&` and `[^]` the exact `message`.
- [ ] **Step 2: Run** `cargo test -p ridl-sem pattern::` — expected: FAIL on the
      first `REFUSED` entry.
- [ ] **Step 3: Implement** `check`.
- [ ] **Step 4: Run** `cargo test -p ridl-sem pattern::` and
      `cargo clippy -p ridl-sem --all-targets -- -D warnings` — expected: PASS.

**Mutations the tests must catch:** delete the refusal of `&&` inside a class
(`[a&&b]` passes and fails the table); delete the `[^]` check; accept `\-`
outside a class; skip the `{n,m}` order check; let `^` through as a literal.

- [ ] **Step 5: Commit message**

```text
feat(ridl-sem): add a recogniser for typl patterns as I-Regexp

The recogniser accepts the RFC 9485 grammar and refuses the six spellings
that the engines read differently, [^], a reversed class range and a
quantifier whose maximum is below its minimum. Each refusal names the
construct and its replacement. Nothing calls it yet.

Refs #597
```

### Task 3: `matches` in the IR and the codegen model, every reader moved

The meaning does not change in this task: each clause is still searched with
today's text.

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto:214-231` and
  `crates/ridl-ir/proto/ridl/codegen/v1/model.proto:212-222`
- Modify: `crates/ridl-ir/src/pattern.rs` (clause helpers)
- Modify (writer): `crates/ridl-sem/src/check.rs:2187-2224`
  (`lower_len_scalar`), `:2885-2906` (`named_string_constraint`), `:2937-2953`
  (`string_pattern`), `:2383-2425` (`check_string_init`)
- Modify (readers): `crates/ridl-sem/src/init.rs:170-175`;
  `crates/ridl-ir/src/lib.rs:1114-1120` (`walk_type_def`), `:1149-1158`
  (`constraint_is_vacuous`); `crates/ridl-ir/src/rules.rs:199-201`;
  `crates/ridl-ir/src/catalog_hash.rs:495-504`;
  `crates/ridl-ir/src/codegen/lower.rs:1817-1830`;
  `crates/ridl-ir/src/codegen/wire.rs:165-173`;
  `crates/ridl-backend-rust/src/lib.rs:1658-1702`;
  `crates/ridl-backend-rust/src/codec_proto3.rs:2617-2621`;
  `crates/ridl-backend-ts/src/lib.rs:532-552` and `:690-696`;
  `crates/ridl-backend-proto/src/lib.rs:1378-1382`;
  `crates/ridl-backend-flatbuffers/src/lib.rs:1502-1506`;
  `crates/ridl-descriptor/src/lower.rs:510-514`;
  `crates/ridl-lsp/src/hover.rs:869-874`;
  `crates/ridlc/src/design_lints/shapes.rs:118-124`
- Not changed: `crates/ridlc/src/deps.rs:82-140` walks the JSON recursively and
  already finds a `patternConst` key inside `matches`; it gets a test only.
  `crates/ridl-diff` is Task 4.
- Modify: `freeze/codegen-protocol.txt` through `cargo xtask freeze update`
- Modify: `docs/specification/ir-specification.md` §6 (one paragraph, below)

**Interfaces:**

- Proto, both packages:

```proto
// One `match` clause (typl §5.3).
message Match {
  // ridl.ir.v2: the source literal with its delimiters.
  // ridl.codegen.v1: the regex body, `/…/` stripped.
  optional string pattern = 1;
  // The canonical reference to the regex constant, when the clause names one.
  optional string pattern_const = 2;
}
```

`Constraint` gains `repeated Match matches = 8;`, and fields 6 and 7 gain
`[deprecated = true]` with a comment: "Deprecated: the first clause of
`matches`, still written and read (IR specification §6)." (Task 6 rewrites the
comments to the final meaning.)

- `ridl_ir::pattern`:

```rust
pub struct MatchClause<'a> { pub pattern: Option<&'a str>, pub pattern_const: Option<&'a str> }
pub struct MatchClauses<'a> { pub clauses: Vec<MatchClause<'a>>, pub legacy_search: bool }
pub fn ir_match_clauses(c: &crate::v2::Constraint) -> MatchClauses<'_>;
pub fn model_match_clauses(c: &crate::codegen::v1::Constraint) -> MatchClauses<'_>;
```

When `matches` is non-empty, `clauses` holds them in order and `legacy_search`
is false. When it is empty and `pattern` or `pattern_const` is present,
`clauses` holds that one clause and `legacy_search` is true. Otherwise both are
empty and false. Every reader of this task uses one of the two functions; none
reads `pattern` or `pattern_const` of `Constraint` directly.

Behaviour of each site:

- Writer (`lower_len_scalar`, `named_string_constraint`): `matches` holds one
  `Match` with today's `pattern` (the literal) and `pattern_const`; fields 6 and
  7 keep today's values. `string_pattern` returns
  `Vec<(Option<String>,
  Option<String>)>` (one entry for now).
- `check_string_init`, `init::string_init`, `constraint_is_vacuous`,
  `codec_proto3` absent rule: a pattern is present when `clauses` is non-empty.
  `check_string_init` tests every clause with today's `regress` search.
- `walk_type_def`, `catalog_hash::visit_type_def`, `shapes.rs`: visit
  `pattern_const` of every element of `matches` and of the deprecated field.
  `visit_type_def` and `shapes.rs` take `&mut`, so iterate
  `c.matches.iter_mut()` and then the deprecated field.
- `rules.rs`: one `Rule::Pattern(text)` per clause, in order, text = the
  clause's `pattern` (the author's literal).
- `codegen/lower.rs::constraint`: model `matches[i].pattern` =
  `strip_regex_delimiters(ir pattern)`, `pattern_const` copied; deprecated
  fields as today. A 0.20 IR (legacy) leaves model `matches` empty and copies
  the deprecated fields as today.
- `codegen/wire.rs::scalar_type`: copy `matches` verbatim.
- Rust backend: one `#[cfg(feature = "validate-pattern")]` block per clause with
  a pattern, each with its own `static PATTERN`, in clause order, today's
  `Regex::new(#source)` text and `is_match`.
- TypeScript backend: one constant per clause, `__ridlPattern{name}_{index}`
  (index from 0), and one `Check` per clause in order; constraint text
  `/{body}/` as today.
- proto3, FlatBuffers, descriptor, hover: write every clause in order, each as
  `match {pattern_const}` when the clause names a constant, else
  `match {pattern}`; hover joins them with a space after the length bound (today
  it returns early on the bound; keep the bound and append the clauses).
- Prost marks the deprecated fields `#[deprecated]`. Where a struct literal sets
  both to `None`, replace the two lines with `..Default::default()`; put
  `#[allow(deprecated)]` on each function that still sets or reads them (the
  writer, `codegen/lower.rs::constraint`, the two clause helpers, and tests that
  build a 0.20 input).

- [ ] **Step 1: Write the failing tests.**
  - `ridl-ir` `pattern::tests`: the three cases of `ir_match_clauses` (matches,
    legacy, none) and of `model_match_clauses`.
  - `ridl-sem` `check.rs` tests: `type Vin : string [17 match /[A-Z]{17}/]`
    lowers to
    `matches == [Match { pattern: Some("/[A-Z]{17}/"), pattern_const:
    None }]`;
    through `const VIN = /…/` the clause has `pattern_const:
    Some("VIN")`.
  - `ridl-ir` `lib.rs` `vacuous_constraint` (`:2904`): a constraint with only
    `matches` is not vacuous; the reference walk finds a constant named only in
    `matches[1]`.
  - `catalog_hash.rs` (next to `:1540`): `matches[1].pattern_const` is
    qualified.
  - `rules.rs`: two clauses give two `Rule::Pattern` entries in order.
  - `ridlc/tests/design_lints.rs` (next to `:1099`) and a `deps.rs` unit test: a
    constant named in `matches[1]` is found.
  - `ridl-backend-rust/src/tests.rs`: a model scalar with two clauses emits two
    `Regex::new` calls in order; `ridl-backend-ts/src/tests.rs`: two
    `__ridlPattern{name}_` constants; proto, FlatBuffers, descriptor and hover:
    two clauses render as `[0..8 match /a/ match P]` (each crate's existing
    constraint-spelling test file).
  - Legacy: each of the Rust backend, the TypeScript backend and
    `ridl_ir::codegen::lower` given a constraint with `pattern` and no `matches`
    emits what it emits today (a test per crate).
- [ ] **Step 2: Run** each crate's tests (`cargo test -p ridl-ir`,
      `-p ridl-sem`, `-p ridl-backend-rust --lib`, `-p ridl-backend-ts --lib`,
      `-p ridl-backend-proto`, `-p ridl-backend-flatbuffers`,
      `-p ridl-descriptor`, `-p ridl-lsp`, `-p ridlc --test design_lints`,
      `-p ridlc --lib deps`) — expected: the new tests FAIL by assertion.
- [ ] **Step 3: Implement** the proto change, the helpers, and every site.
- [ ] **Step 4: Run** the same commands, then
      `INSTA_UPDATE=always cargo test -p ridlc --test corpus` and
      `INSTA_UPDATE=always cargo test -p ridl-backend-ts --test codec`, then
      `cargo test --workspace --locked` and
      `cargo clippy --workspace --all-targets -- -D warnings` — expected: PASS.
      Snapshot diffs show only added `matches` entries and changed catalog
      hashes.
- [ ] **Step 5: Freeze.** Run `cargo xtask freeze update`. Expected: only
      `freeze/codegen-protocol.txt` changes, with these six added lines and none
      removed:
      `ridl.codegen.v1.Constraint.matches = 8 repeated ridl.codegen.v1.Match`,
      `ridl.codegen.v1.Match.pattern = 1 optional string`,
      `ridl.codegen.v1.Match.pattern_const = 2 optional string`, and the same
      three for `ridl.ir.v2`. Run `cargo xtask freeze check`. Report any other
      changed dump.
- [ ] **Step 6: IR specification.** In `docs/specification/ir-specification.md`
      §6, after the table, add: "One exception is recorded. `Constraint.matches`
      holds every `match` clause of a string type, and the deprecated
      `Constraint.pattern` and `pattern_const` hold the first. A reader that
      reads only the deprecated fields checks the first clause of a type with
      several clauses and ignores the others. The owner accepted this for this
      one source feature (driftsys/ridl#597)."

**Mutations the tests must catch:** read only `matches[0]` in the Rust backend
(the two-call test fails); skip `matches` in `catalog_hash::visit_type_def` (the
qualification test fails); drop the legacy fallback in `model_match_clauses`
(the legacy test fails); make `constraint_is_vacuous` ignore `matches`.

- [ ] **Step 7: Commit message**

```text
feat(ridl-ir): carry every match clause in a repeated matches field

ridl.ir.v2.Constraint and ridl.codegen.v1.Constraint gain repeated Match
matches. The deprecated pattern and pattern_const fields are still written
for the first clause and still read when matches is empty, as input from
0.20 has it. Every reader in the workspace moves to the clause list.

Refs #597
```

### Task 4: `ridl diff` compares clause sets

**Files:**

- Modify: `crates/ridl-diff/src/classify.rs:631-637` (in `narrows`, `:601`)
- Modify: `crates/ridl-diff/src/walk.rs:1935-1937` (in `constraint_str`,
  `:1914`)
- Test: `crates/ridl-diff/src/classify/classify_tests.rs` (next to
  `a_changed_pattern_is_breaking`, `:860`)

**Interfaces:** consumes `ridl_ir::pattern::ir_match_clauses` (Task 3).

Behaviour: a clause is the pair `(pattern, pattern_const)`. `narrows` is true
when the new side holds a clause that the old side does not hold. Order does not
count; a removed clause widens. `constraint_str` writes one `pattern=<text>`
part per clause, in order.

- [ ] **Step 1: Write the failing tests:** adding a second clause narrows;
      removing one does not; swapping two does not; an old side that is a 0.20
      constraint (deprecated `pattern = "/a/"`, empty `matches`) against a new
      side with `matches = [/a/]` does not narrow; a rewritten clause narrows.
- [ ] **Step 2: Run** `cargo test -p ridl-diff` — expected: the add and the 0.20
      tests FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p ridl-diff` and
      `cargo test -p ridl-cli --test diff_cli` — expected: PASS.

**Mutations the tests must catch:** compare the clause lists in order (the swap
test fails); compare only the first clause (the add test fails).

- [ ] **Step 5: Commit message**

```text
feat(ridl-diff): compare match constraints as sets of clauses

A clause the new side adds narrows the type and a clause it removes widens
it. The order of the clauses has no meaning. Input from 0.20 is read as one
clause.

Refs #597
```

### Task 5: Several `match` clauses in the source

**Files:**

- Modify: `crates/ridl-syntax/src/parser.rs:2434-2440` (bare `match`, loop while
  at `match`), `:2516-2522` (`match_opt` loops); the `MatchKw` look-ahead arms
  at `:2458`, `:2471` stay
- Modify: `crates/ridl-syntax/family.ungram:99-100` to
  `| len:Bound ('match' match_pattern:Literal)*` and
  `| ('match' match_pattern:Literal)+`; regenerate with `cargo xtask codegen`
- Modify: `crates/ridl-syntax/src/ast.rs:670-674`: replace `match_pattern()` by
  `pub fn match_patterns(&self) -> impl Iterator<Item = Literal>` (every direct
  `Literal` child after a `MatchKw` token, in order); update the ast tests at
  `:1008`, `:1225`, `:1236`, `:1241-1310`
- Modify: `crates/ridl-sem/src/check.rs:2189-2213` (one `Match` per clause, each
  inline literal validated as today), `:2843-2848`
  (`match_patterns().next().is_none()`), `:2937-2953` (`string_pattern` returns
  every clause); the deprecated fields hold the first clause
- Test: `crates/ridl-fmt` (layout is already token-driven at
  `crates/ridl-fmt/src/lib.rs:1661-1690`; add the test only)
- Test: `crates/ridl-backend-rust/tests/fixtures/payload_constraints.ridl:30`
  and `crates/ridl-backend-rust/tests/payload_constraints.rs`;
  `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl` and `ctor_test.ts`
- Modify: `docs/specification/typl-language-reference.md` §5.3 (several clauses,
  the four bullets of the note §4, the example
  `type Token : string [8..64 match /.*[0-9].*/ match /.*[A-Z].*/]` written with
  today's dialect as `/[0-9]/` and `/[A-Z]/` until Task 7 rewrites it) and
  Appendix E (`('match' pattern)*` after a length bound, `('match' pattern)+`
  alone)

- [ ] **Step 1: Write the failing tests.**
  - parser corpus: `crates/ridl-syntax/test_data/parser/ok/several_match.typl`
    with `type A : string [8..64 match /a/ match /b/]` and
    `type B : string [match /a/ match P]`; error-free, lossless, new snapshot.
  - ast: `match_patterns()` returns two literals in order.
  - ridl-fmt: `type T:string[1..8 match /a/ match /b/]` formats to
    `type T : string [1..8 match /a/ match /b/]` and is idempotent.
  - ridl-sem: `type T : string [1..8 match /[0-9]/ match /[A-Z]/] = "1a"` is
    TYPL-109 and `= "1A"` is clean; the IR has two `matches`, the deprecated
    `pattern` is `/[0-9]/`; a type with two clauses is TYPL-115 when it has no
    init; an invalid second inline clause `/[/` is TYPL-106.
  - Rust payload: add
    `struct Twice { text : string [2..2 match /[0-9]/ match
    /[A-Z]/] }` to
    the fixture; `"1a"` is refused with `Rule::Pattern` and `"1A"` is accepted.
  - TypeScript ctor: add
    `type Twice : string [0..8 match /[0-9]/ match
    /[A-Z]/]`;
    `makeTwice("1a")` throws a `ConstraintError` whose message names `/[A-Z]/`;
    `makeTwice("1A")` returns.
- [ ] **Step 2: Run** `cargo test -p ridl-syntax`, `cargo test -p ridl-fmt`,
      `cargo test -p ridl-sem`,
      `cargo test -p ridl-backend-rust --test
      payload_constraints` —
      expected: FAIL (the second `match` is a parse error today).
- [ ] **Step 3: Implement**, then `cargo xtask codegen`.
- [ ] **Step 4: Run** the same commands plus `cargo test -p xtask codegen` (the
      drift test), `cargo test -p ridl-cli --test book_examples`, and
      `cargo clippy --workspace --all-targets -- -D warnings` — expected: PASS.
      The ctor TypeScript test runs under `just cabin-ws-demo` in CI; the
      sandbox may not have `flatc`, so report whether it ran.

**Mutations the tests must catch:** stop the `match_opt` loop after one clause
(the parser test fails); lower only the first clause in `lower_len_scalar` (the
`"1a"` init test fails); test only the first clause in `check_string_init`.

- [ ] **Step 5: Commit message**

```text
feat(typl): accept several match clauses on a string type

A string constraint takes match zero or more times after a length bound,
and one or more times alone. A value must match every clause, and the order
of the clauses has no meaning.

Refs #597
```

### Task 6: Every output matches the whole value

**Files:**

- Modify: `crates/ridl-ir/src/codegen/lower.rs:1817-1830`: model
  `matches[i].pattern = normalised_pattern(ir literal)`; deprecated model
  `pattern = search_engine_text(normalised_pattern(first literal))`. The 0.20
  input path (legacy) is unchanged.
- Modify: `crates/ridl-sem/src/check.rs` writer sites of Task 3: deprecated IR
  `pattern = deprecated_ir_literal(first literal)`; `matches[i].pattern` stays
  the literal. The init check in `check_string_init` still searches each clause
  literal with `regress` (Task 7 changes it).
- Modify: `crates/ridl-backend-rust/src/lib.rs:1658-1702`: for a non-legacy
  clause emit `Regex::new(#text)` with `text = rust_engine_text(pattern)`; a
  legacy clause keeps today's text. Rewrite the comment at `:1684-1686`.
- Modify: `crates/ridl-backend-ts/src/lib.rs:532-552`, `:690-696`: for a
  non-legacy clause,
  `new globalThis.RegExp(ts_string(search_engine_text(
  normalised_pattern(literal))), "u")`;
  a legacy clause keeps `new globalThis.RegExp(body)` with no flag. The
  `ConstraintError` text stays the author's literal.
- Modify: proto comments of Task 3 to the final meaning: `Match.pattern` in
  `ridl.codegen.v1` is "the normalised pattern (`ridl_ir::pattern`), matched
  against the whole value"; deprecated `pattern` in `ridl.codegen.v1` is
  "`^(?:P)$`, a search text equal to the whole-value match of the first clause";
  in `ridl.ir.v2`, "`/^(?:S)$/`".
- Migrate (their tests rely on a search):
  - `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl:64`
    `type Contains : string [0..16 match /ab/]` →
    `type Contains : string [0..16 match /.*ab.*/]`
  - `examples/cabin/cabin.ridl:116`
    `type Digited: string [0..16 match /\d/] = "1"` →
    `type Digited: string [0..16 match /.*[0-9].*/] = "1"`
- Create: `crates/ridl-ir/tests/pattern_agreement.json` and
  `crates/ridl-ir/tests/pattern_agreement.rs`
- Modify: `examples/cabin/web/check_test.ts` (one more `Deno.test`, and values
  for `Glyph` and `Digited` at `:560-575`)
- Modify: `freeze/generated/veh-cluster/*.rs` through
  `cargo xtask freeze update`

**Interfaces:** consumes every function of Task 1 and the clause helpers of
Task 3.

The agreement table is a JSON array of
`{ "literal": "/…/", "normalised": "…", "value": "…", "accepted": bool }`. It
holds: for each `ridl.std` pattern of Task 7's "After" column one accepted and
one refused value; for `/[0-9]{4}-[0-9]{2}-[0-9]{2}/` the values `"2026-11-18"`
(true), `"x2026-11-18"`, `"2026-11-18\n"`, `"١٢٣٤-١٢-١٢"` (false); for `/./` the
values `"a"`, `"é"`, `"\u{1F600}"`, `" "`, `" "` (true; the normalised `[^\n\r]`
accepts U+2028 and U+2029 in both engines), `"\n"`, `"\r"`, `""`, `"ab"`
(false); for `/[^a]/` the value `"\u{1F600}"` (true); for `/a|bc/` `"a"`, `"bc"`
(true), `"abc"` (false); for `/[\--z]/` `"-"`, `"z"` (true); for `/\^[$]/`
`"^$"` (true); for `/[a&b]/` `"&"` (true).

- [ ] **Step 1: Write the failing tests.**
  - `pattern_agreement.rs`: for every row,
    `normalised_pattern(literal) ==
    normalised`;
    `Regex::new(&rust_engine_text(&n))?.is_match(value) ==
    accepted`; and
    the search with the deprecated text,
    `Regex::new(&search_engine_text(&n))?.is_match(value) == accepted`.
  - `check_test.ts`:
    `Deno.test("the RegExp engine text agrees with the
    table")` reads the
    JSON
    (`new URL("../../../crates/ridl-ir/tests/
    pattern_agreement.json", import.meta.url)`)
    and asserts `new RegExp("^(?:" + n + ")$", "u").test(value) === accepted`
    per row. Add `"\r"`, `" "` and `"\u{1F600}"` to `Glyph`'s values, and `"١"`
    and `"a\n1"` to `Digited`'s.
  - `ridl-backend-rust/src/tests.rs`: a model clause `[0-9]+` emits
    `Regex::new("\\A(?:[0-9]+)\\z")`; a legacy constraint emits today's text.
    Update the existing assertions at `src/tests.rs:271`, `:308`, `:334`,
    `:1368`, `:1401`, near `:1725`, near `:3985` and `:7495` that pin the
    emitted text.
  - `ridl-backend-ts/src/tests.rs`: `string_type("Vin", "/[A-Z]{17}/")` in a
    non-legacy IR (`matches` set) emits
    `new globalThis.RegExp("^(?:[A-Z]{17})$", "u")`; update the assertions at
    `:1658`, `:2418`, `:2939`, `:2979`, `:3003`, `:3016` as the new text
    requires.
  - `ridl-ir` `codegen` tests: the model of `/a.b/` has
    `matches[0].pattern ==
    "a[^\\n\\r]b"` and deprecated
    `pattern == "^(?:a[^\\n\\r]b)$"`; ridl-sem: deprecated IR
    `pattern == "/^(?:[A-Z]{17})$/"`.
  - ctor_test.ts: `makeContains("xxabxx")` still returns; add `makeDot(" ")`
    agrees with the wasm codec (the existing differential test).
- [ ] **Step 2: Run** `cargo test -p ridl-ir`,
      `cargo test -p
      ridl-backend-rust --lib`,
      `cargo test -p ridl-backend-ts --lib` — expected: FAIL by assertion.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same, `cargo test -p ridl-sem`, `cargo test -p ridlc`,
      `INSTA_UPDATE=always cargo test -p ridlc --test
      corpus`,
      `INSTA_UPDATE=always cargo test -p ridl-backend-ts --test
      codec`,
      `cargo test --workspace --locked`,
      `cargo clippy --workspace --all-targets -- -D warnings`. The Deno tests
      run in CI (`just cabin-ws-demo`); report whether the sandbox ran them.
- [ ] **Step 5: Freeze.** `cargo xtask freeze update`; expected changes: the
      `Regex::new` lines of `freeze/generated/veh-cluster/ridl.std.rs`,
      `veh.common.rs`, `veh.cluster.rs`. Then `cargo xtask freeze check`; report
      its output.

**Mutations the tests must catch:** drop the `.` rewrite in the lowering (the
`\r` row fails in Rust); emit `Regex::new(P)` without the anchors (the
`"x2026-11-18"` row and the Rust emit test fail); drop the `"u"` flag (the
`\u{1F600}` row of `/[^a]/` fails in Deno); wrap the legacy clause too (the
legacy tests fail).

- [ ] **Step 6: Commit message**

```text
feat(ridl-ir)!: match a pattern against the whole value in every output

The codegen model carries the normalised pattern of each clause. The Rust
output compiles \A(?:P)\z and the TypeScript output builds
RegExp("^(?:P)$", "u"). The deprecated pattern fields hold a search text
that gives the same verdict. Input written by 0.20 keeps its search.

BREAKING CHANGE: a match pattern with no anchor now accepts only a value
that it matches from the first character to the last, in the generated Rust
and TypeScript code. /ABC/ accepted "xABCx" and now accepts only "ABC". Write
.*ABC.* for a search.

Refs #597
```

### Task 7: The checker accepts I-Regexp only (the breaking switch)

**Files:**

- Modify: `crates/ridl-sem/src/check.rs:1830-1869` (`validate_regex`): call
  `pattern::check(&ridl_ir::pattern::pattern_text(raw))`; on `Err(r)` emit
  TYPL-106 with `r.message` on the literal's range; otherwise
  `regex_crate_verdict(&rust_engine_text(&normalised_pattern(raw)))`, TYPL-220
  with the message `the Rust \`regex\` crate cannot compile this pattern:
  {reason} (typl §2.7)`.
  Rewrite the doc comment (no ECMA-262, no`regress`).
- Modify: `crates/ridl-sem/src/check.rs:6311-6330` (`regex_crate_refusal`), the
  size-limit text:
  `the compiled pattern exceeds the crate's size limit of
  {limit} bytes; a counted repetition inside another counted repetition is the
  usual cause`.
- Modify: `crates/ridl-sem/src/check.rs:2416-2425` (`check_string_init`): for
  each clause literal,
  `regex::Regex::new(&rust_engine_text(
  &normalised_pattern(lit)))` and
  `is_match(text)`; a refused value reports the clause: `init string \`{text}\`
  does not match the type's \`match\` pattern {lit}`.
- Modify: `crates/ridl-sem/src/check.rs:6242-6246`: delete `regex_body` if
  nothing uses it any more.
- Remove `regress`: `crates/ridl-sem/Cargo.toml:25`, the root `Cargo.toml:144`
  and its comment at `:145-155` (rewrite: "ridl-sem compiles every accepted
  pattern with `regex` in the engine text of the Rust output, so a pattern that
  crate refuses is TYPL-220"). Run `cargo check -p ridl-sem` and confirm that
  `Cargo.lock` lost `regress` and its now unused dependencies, and nothing else.
- Modify: `crates/ridl-core/src/diag.rs:390-393` and `:526-530`: TYPL-106 "A
  pattern that is not a typl I-Regexp (typl §2.7)"; TYPL-220 "A `match` pattern
  or regex `const` that the checker accepts and the Rust `regex` crate cannot
  compile, such as one over the crate's size limit". Keep the catalogue titles.
- Modify: `crates/ridl-sem/tests/regex_features.rs:34` and `:48`: the messages
  name `\w` and `\p{..}`; leave the feature check, replace the parenthesis by
  "(TYPL-220 compiles patterns with non-ASCII characters only with `unicode`)"
  if the test still passes; if it fails because a probe pattern is now refused,
  stop and report.
- Rewrite the tests of `check.rs` listed in the migration table.
- Migrate every literal in the table below in this task.

The breaking change: from this task every pattern in the left column is
TYPL-106. The table is the complete list of tracked pattern literals that the
checker reads and that this task changes.

| File:line                                                                                   | Old                                                                                    | New                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/ridl-core/assets/ridl_std.typl:6`                                                   | `/^[0-9a-f]{8}-…-[0-9a-f]{12}$/`                                                       | the same text without `^` and `$`                                                                                                                                                                                                                                  |
| `crates/ridl-core/assets/ridl_std.typl:8`                                                   | `/^[0-7][0-9A-HJKMNP-TV-Z]{25}$/`                                                      | `/[0-7][0-9A-HJKMNP-TV-Z]{25}/`                                                                                                                                                                                                                                    |
| `crates/ridl-core/assets/ridl_std.typl:10`                                                  | `/^[a-zA-Z][a-zA-Z0-9+\-.]*:\/\/.+$/`                                                  | `/[a-zA-Z][a-zA-Z0-9+\-.]*:\/\/.+/`                                                                                                                                                                                                                                |
| `crates/ridl-core/assets/ridl_std.typl:12`                                                  | `/^https?:\/\/.+$/`                                                                    | `/https?:\/\/.+/`                                                                                                                                                                                                                                                  |
| `crates/ridl-core/assets/ridl_std.typl:14`                                                  | `/^[^@]+@[^@]+\.[^@]+$/`                                                               | `/[^@]+@[^@]+\.[^@]+/`                                                                                                                                                                                                                                             |
| `crates/ridl-core/assets/ridl_std.typl:16`                                                  | `/^(\d{1,3}\.){3}\d{1,3}$/`                                                            | `/([0-9]{1,3}\.){3}[0-9]{1,3}/`                                                                                                                                                                                                                                    |
| `crates/ridl-core/assets/ridl_std.typl:18`                                                  | `/^[0-9a-f:]+$/`                                                                       | `/[0-9a-f:]+/`                                                                                                                                                                                                                                                     |
| `crates/ridl-core/assets/ridl_std.typl:20`                                                  | `/^[\x20-\x7E]+$/`                                                                     | `/[ -~]+/`                                                                                                                                                                                                                                                         |
| `crates/ridl-core/assets/ridl_std.typl:70`                                                  | `/^\d{4}-\d{2}-\d{2}$/`                                                                | `/[0-9]{4}-[0-9]{2}-[0-9]{2}/`                                                                                                                                                                                                                                     |
| `crates/ridl-core/assets/ridl_std.typl:73`                                                  | `/^\d{2}:\d{2}:\d{2}/`                                                                 | `/[0-9]{2}:[0-9]{2}:[0-9]{2}.*/`                                                                                                                                                                                                                                   |
| `crates/ridl-core/assets/ridl_std.typl:78`                                                  | `/^\d+\.\d+\.\d+/`                                                                     | `/[0-9]+\.[0-9]+\.[0-9]+.*/`                                                                                                                                                                                                                                       |
| `crates/ridl-core/assets/ridl_std.typl:83`                                                  | `/^[A-Z]{2}$/`                                                                         | `/[A-Z]{2}/`                                                                                                                                                                                                                                                       |
| `crates/ridl-core/assets/ridl_std.typl:86`                                                  | `/^[a-z]{2}(-[A-Z]{2})?$/`                                                             | `/[a-z]{2}(-[A-Z]{2})?/`                                                                                                                                                                                                                                           |
| `examples/cabin/cabin.ridl:110`                                                             | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `examples/cabin/cabin.ridl:113`                                                             | `/^.$/`                                                                                | `/./`                                                                                                                                                                                                                                                              |
| `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl:61`                                   | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl:67`                                   | `/^.$/`                                                                                | `/./`                                                                                                                                                                                                                                                              |
| `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl:70`                                   | `/^\d+'x$/`                                                                            | `/[0-9]+'x/`                                                                                                                                                                                                                                                       |
| `crates/ridl-backend-ts/tests/fixtures/ctor/ctor.ridl:85`                                   | `/^[a-z]*$/`                                                                           | `/[a-z]*/`                                                                                                                                                                                                                                                         |
| `crates/ridl-backend-ts/tests/fixtures/ctor/ctor_test.ts:397`                               | `pattern rule /^[A-Z]+$/`                                                              | `pattern rule /[A-Z]+/`                                                                                                                                                                                                                                            |
| `crates/ridlc/tests/corpus/rust-shapes/shapes.ridl:23`                                      | `/^[a-z][a-z0-9-]*$/`                                                                  | `/[a-z][a-z0-9-]*/`                                                                                                                                                                                                                                                |
| `crates/ridlc/tests/corpus/diag-showcase/scalars.typl:38`                                   | `/^[a-z]{3,8}$/`                                                                       | `/[a-z]{3,8}/`                                                                                                                                                                                                                                                     |
| `crates/ridlc/tests/corpus/ridl-diag-showcase/main/vocabulary.typl:14`                      | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `crates/ridlc/tests/corpus/ridl-diag-showcase/main/regex_patterns.typl:1-12`                | header names TYPL-220 only; `type LookaheadCode : string [1..8 match /^(?=A)[A-Z]+$/]` | header: "TYPL-106 for a construct outside the language, TYPL-220 for an accepted pattern over the `regex` crate's size limit"; `type NestedCode : string [1..8 match /(([a-z]{1,64}){1,64}){1,64}/]` (TYPL-220); keep `REPEATED_PATTERN = /^(a)\1$/`, now TYPL-106 |
| `crates/ridlc/tests/design_lints.rs:1099`                                                   | `/^x+$/`                                                                               | `/x+/`                                                                                                                                                                                                                                                             |
| `crates/ridl-lsp/tests/server.rs:1437`                                                      | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `crates/ridl-mcp/tests/fixtures/ws/a/a.ridl:29`, `ws-v2/a/a.ridl:30`, `ws-diag/a/a.ridl:31` | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `docs/book/getting-started.md:426`                                                          | `/^[A-Z][A-Z0-9]{2,5}$/`                                                               | `/[A-Z][A-Z0-9]{2,5}/`                                                                                                                                                                                                                                             |
| `docs/book/getting-started.md:428`                                                          | `/^[A-HJ-NPR-Z0-9]{17}$/`                                                              | `/[A-HJ-NPR-Z0-9]{17}/`                                                                                                                                                                                                                                            |
| `crates/ridl-sem/src/check.rs:10012` and the expectation `:10039`                           | `/^[A-Z0-9]{17}$/`                                                                     | `/[A-Z0-9]{17}/`                                                                                                                                                                                                                                                   |
| `crates/ridl-sem/src/check.rs:10174`                                                        | `/^[A-HJ-NPR-Z0-9]{17}$/`                                                              | `/[A-HJ-NPR-Z0-9]{17}/`                                                                                                                                                                                                                                            |
| `crates/ridl-sem/src/check.rs:10246`                                                        | `/^[A-Z]{2}$/`                                                                         | `/[A-Z]{2}/`                                                                                                                                                                                                                                                       |
| `crates/ridl-sem/src/check.rs:11365`, `:11377`, `:11458`, `:11538`, `:11910`                | `/^[A-Z]+$/`                                                                           | `/[A-Z]+/`                                                                                                                                                                                                                                                         |
| `crates/ridl-sem/src/check.rs:11703`, `:12428`                                              | `/^a+$/`                                                                               | `/a+/`                                                                                                                                                                                                                                                             |
| `crates/ridl-sem/src/check.rs:12420`                                                        | `/^b+$/`                                                                               | `/b+/`                                                                                                                                                                                                                                                             |

Tests of `check.rs` to rewrite, not only migrate:

- `REFUSED_BY_THE_REGEX_CRATE` (`:10195-10229`): becomes
  `[r"/(a{1000}){1000}/", r"/(([a-z]{1,64}){1,64}){1,64}/"]` (both TYPL-220).
  The other entries move to a new `REFUSED_BY_THE_RECOGNISER` table whose tests
  assert TYPL-106 alone, inline and as a `const`, with the anchors removed where
  the anchor is not the construct under test.
- `typl_220_probes_are_ecma_262_the_regex_crate_refuses` (`:10267`): becomes
  "the probes pass the recogniser and the `regex` crate refuses their engine
  text".
- `typl_220_size_limit_message_names_the_large_unicode_classes` (`:10314`):
  pattern `/(([a-z]{1,64}){1,64}){1,64}/`, the new full message.
- `typl_220_keeps_the_regex_crate_verdict_across_checks` (`:10347`): seed the
  key `rust_engine_text("seeded-for-issue-603")` and write the literal
  `/seeded-for-issue-603/`.
- `typl_220_uses_the_regex_configuration_the_rust_backend_emits` (`:10483`):
  probes `/[à-ÿ]+/` and `/.+/`.
- `backend_patterns` (`:10251`): read `matches[].pattern` and compile
  `rust_engine_text` of each in
  `every_pattern_the_checker_passes_compiles_under_the_regex_crate`.
- `typl_106_*` (`:10153`, `:10159`): assert the new message
  `invalid regular expression syntax: unclosed class`.
- New tests: one per row of the note §3.3 (`^`, `&&`, `~~`, `--`, `\-`, `\p{L}`)
  as a TYPL-106 with its message; `type D : ridl.std.Date =
  "x2026-11-18"`,
  `"2026-11-18\n"` and the Arabic-Indic date are TYPL-109, `"2026-11-18"` is
  clean; a two-clause type whose init fails only the second clause is TYPL-109
  and the message names the second literal; an init with `\r` under `/./` is
  TYPL-109.

Not migrated (the checker never reads them; they are lexer, parser or formatter
input, and their snapshots pin syntax only): `crates/ridl-syntax/test_data/**`,
`crates/ridl-syntax/src/{lexer,ast}.rs` tests,
`crates/ridl-fmt/test_data/{input,formatted}/tight_reference.typl:18`, IR-level
test strings that build a `Constraint` by hand
(`crates/ridl-backend-rust/src/tests.rs`, `crates/ridl-backend-ts/src/tests.rs`,
`crates/ridl-backend-ts/examples/constructor_fixture.rs:129`,
`crates/ridl-descriptor/tests/lower.rs:1048-1065`,
`crates/ridl-ir/src/projection/size.rs:1764`,
`crates/ridl-sem/src/init.rs:385`), and the unanchored patterns that stay valid
with their new meaning
(`crates/ridl-backend-rust/tests/fixtures/payload_constraints.ridl:30` `/ABC/`,
`crates/ridl-backend-rust/tests/fixtures/proto3_roundtrip.ridl:49,166`
`/[a-z]+/`). If a test over those two fixtures now fails because it relied on a
search, change the test value, not the pattern, and report it.

- [ ] **Step 1: Write the failing tests** (the new tests above, before any
      migration). Run `cargo test -p ridl-sem` — expected: FAIL (the `^` refusal
      test passes today with no diagnostic).
- [ ] **Step 2: Implement** the check, the init test, the messages and the
      `regress` removal.
- [ ] **Step 3: Migrate** every row of the table; rewrite the listed tests.
- [ ] **Step 4: Run** `cargo test -p ridl-sem`, `cargo test -p ridl-core`,
      `cargo test -p ridl-lsp`, `cargo test -p ridl-mcp`,
      `cargo test -p ridl-cli --test book_examples`,
      `INSTA_UPDATE=always cargo test -p ridlc --test corpus`,
      `INSTA_UPDATE=always cargo test -p ridl-backend-ts --test codec`,
      `cargo test --workspace --locked`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and
      `git grep -n regress -- Cargo.toml Cargo.lock crates` (expected: no
      output). `cargo test -p ridl-mcp` may need a TCP port; report if the
      sandbox refuses it.
- [ ] **Step 5: Freeze.** `cargo xtask freeze update`; expected changes: the
      `*_PATTERN` const values and `/// - pattern` doc lines of
      `freeze/generated/veh-cluster/ridl.std.rs`, `veh.common.rs` and
      `veh.cluster.rs`. Then `cargo xtask freeze check`; report its output.

**Mutations the tests must catch:** remove one refusal of the note §3.3 in
`pattern::check` (its TYPL-106 test fails); test the init value with
`search_engine_text` but without anchors (the `"x2026-11-18"` test fails); skip
the `.` rewrite in the init path (the `\r` test fails); test only the first
clause (the two-clause test fails).

- [ ] **Step 6: Commit message**

```text
feat(typl)!: accept a match pattern only when it is an I-Regexp

A typl pattern is an I-Regexp (RFC 9485) without Unicode categories and
without six spellings that the engines read differently. The checker
refuses every other pattern with TYPL-106, whose message names the construct
and its replacement. An init value is tested against every clause with the
engine text of the Rust output. regress leaves the workspace, and ridl.std,
the examples, the fixtures and the book are migrated.

BREAKING CHANGE: a pattern with no anchor and no refused construct still
compiles, and now matches the whole value: /ABC/ accepted "xABCx" and now
accepts only "ABC"; write .*ABC.* for a search. Every pattern that uses ^ or
$, \d, \w, \s, \b, \x, \u, \p{..}, a lookaround, a backreference, a
non-capturing or named group, a lazy quantifier, an inline flag, a POSIX or
nested class, or a class operator is now TYPL-106; the message names the
replacement. In the Rust output \d no longer accepts non-ASCII digits and .
no longer accepts \r; in the TypeScript output . accepts U+2028 and U+2029
and an astral character counts as one character. The 13 patterns of
ridl.std are rewritten; TimeOfDay and Version keep accepting any suffix
without a line break. The catalog hash of every unit that reaches a pattern
changes. ridl diff against a baseline from 0.20 reports every rewritten
pattern as narrowed; for a pattern that was ^…$ with no construct above,
that report is a false positive.

Refs #597
```

### Task 8: Records

Documentation only. Every statement must match the code of Tasks 1 to 7; the
task reviewer checks each against the code.

**Files and what each says (the note §13):**

- `docs/specification/typl-language-reference.md`: §2.7 (`:253-310`) replaced by
  the language of the note §3.1 to §3.6, with the replacement table of §3.2 and
  the six refusals of §3.3; the example at `:261` becomes
  `/[A-HJ-NPR-Z0-9]{17}/`; §5.3 example `Token` rewritten as `/.*[0-9].*/` and
  `/.*[A-Z].*/`; §6.2 examples `:833-839`; §16.2 TYPL-106 and TYPL-220 rows;
  Appendix A (`:1872-1952`) copies the new `ridl_std.typl` text; Appendix C
  replaces the ECMA-262 row (`:2074`) by RFC 9485; Appendix D and Appendix G
  (`:2329`, `:2370`): a `match` maps to a JSON Schema `pattern` `^(?:P)$`,
  several clauses to an `allOf`; Appendix H glossary entry for "pattern".
- `crates/ridl-syntax/family.ungram:90-100`: the comment says "zero or more
  `match` clauses".
- `docs/decisions/ADR-0007-e1-execution.md` decision 10 (`:111-128`): add an
  `_Amended (2026-11-18)._` paragraph: `regress` is removed; a pattern is an
  I-Regexp decided by the recogniser in `ridl-sem`; TYPL-220 remains for the
  `regex` size limit (driftsys/ridl#597). Read its `## Status` first.
- `docs/decisions/ADR-0013-codegen-backend-scope.md` (`:294`, `:374`): add an
  amendment: the TypeScript pattern check uses `RegExp("^(?:P)$", "u")` over the
  normalised pattern, not the checker's ECMA-262 semantics. Read its `## Status`
  first.
- `docs/book/generated-code.md:985-1000`: the `validate-pattern` feature and the
  TypeScript `RegExp` section state the engine texts of the note §7 and that
  both outputs give one verdict.
- `docs/book/getting-started.md` prose around the two constants and `match`
  examples (the fences were migrated in Task 7).
- `docs/book/documenting.md`, `docs/book/codegen-plugins.md`: the rule text of a
  pattern is one `Rule.pattern` per clause, the author's literal; the model
  holds `Constraint.matches` with the normalised pattern; the deprecated
  `pattern` holds `^(?:P)$` for the first clause.
- `docs/design/ts-codec-table.md:558`: the pattern check is
  `RegExp("^(?:P)$", "u")`.
- `docs/design/catalog-descriptor.md`: the two statements that the narrowing of
  a byte capacity from an ASCII-only pattern (driftsys/ridl#665) waits for
  driftsys/ridl#597 now say that #597 is done and #665 is open.
- `docs/specification/ridl-family-overview.md`: follow its footer list of
  sections to update when a reference changes.

- [ ] **Step 1:** Write the text.
- [ ] **Step 2: Run** `prim fmt` on each changed Markdown file, then
      `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `cargo test -p ridl-cli --test book_examples` —
      expected: PASS.
- [ ] **Step 3: Commit message**

```text
docs(typl): state the I-Regexp pattern language in every record

The typl reference, the grammar, ADR-0007 and ADR-0013, the book and the
design records now state that a match pattern is an I-Regexp matched
against the whole value, with several clauses per string type.

Refs #597
```
