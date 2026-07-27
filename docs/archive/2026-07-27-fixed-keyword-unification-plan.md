# `final` to `fixed` Keyword Unification — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename ridl's `final` interaction keyword to `fixed`, so the family
uses one word for the provisioned constant at both the system and the user
boundary.

**Architecture:** Two forced-atomic code changes followed by three document
changes. The IR message rename (task 1) is independent of the source keyword and
lands first. The surface rename (task 2) must be atomic across the lexer, the
parser, the checker, every `.ridl` fixture, and the compiled book examples,
because the workspace neither compiles nor parses its own corpus in any
intermediate state. Tasks 3 to 6 carry no compilation risk.

**Tech Stack:** Rust workspace (`cargo`, `insta` snapshots, `prost`/`protox` for
the IR), `mdbook` for the book, `just` for every gate, `prim` and `markdownlint`
for Markdown.

**Design:** `docs/wip/2026-07-27-fixed-keyword-unification-design.md`

## Global Constraints

- The IR field number `20` does not change. Only the message and field **names**
  change (ADR-0008 decision 8 — the number is the compatibility contract).
- Diagnostic **codes** do not change. RIDL-106, RIDL-301, and FORM-102 keep
  their identifiers; only their message text changes.
- `final` is removed from the reserved-word registry entirely. It is not
  retained as a retired-but-reserved word, and no alias, shim, or migration
  diagnostic is added.
- `docs/archive/` and `docs/decisions/` are **not** rewritten. Archived plans
  and the v0.1 reference are verbatim history; an ADR recording a past decision
  about the word `final` keeps its text. The single exception is the superseding
  pointer added to ADR-0008 in task 5.
- **Never run a global `sed` for `final`.** The word appears as ordinary English
  in code that must not change. Known sites, all of which mean "last" and must
  be left alone:
  - `crates/ridl-lsp/src/nav.rs` — "final segment" (lines 67, 102, 276, 281)
  - `crates/ridl-lsp/src/rename.rs` — "final segment" (lines 16, 150, 296)
  - `crates/ridl-lsp/src/complete.rs:324,334` — "the final segment"
  - `crates/ridl-backend-rust/src/lib.rs:55` — "as a final guard"
  - `crates/ridl-syntax/src/parser.rs:262` — "plus a final entry for the end"
- Prose follows the repository rule: plain and literal, no idioms.
- Every Markdown change is followed by `just fmt`, and `just check` must pass.
- Commit scopes are an explicit list in `.git-std.toml`, not path-derived. The
  crate scopes are `ridl-syntax`, `ridl-core`, `ridl-ir`, `ridlc`,
  `ridl-backend-rust`, `ridl-backend-ts`, `ridl-diff`, `ridl-fmt`, `ridl-sem`,
  `ridl-lsp`, `xtask`, and `editors`; the document scopes are `typl`, `ridl`,
  `uxdl`, `rmdl`, `rsdl`, `family`, `roadmap`, `adr`, `docs`, and `repo`.
  `git std lint` rejects anything else, in a pre-commit hook.
- **The per-task file lists are a starting point, not the full set.** Task 1
  found five consumers its list omitted. Before committing any task, grep for
  the identifiers that task renames and confirm nothing is left behind.

---

### Task 1: Rename the IR message and its Rust consumers

The proto message name is independent of the source keyword, so this task lands
on its own with the surface language unchanged.

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto:103,488-491`
- Modify: `crates/ridl-sem/src/check.rs:2943,3909,3951`
- Modify: `crates/ridl-backend-rust/src/interact.rs:554`
- Modify: `crates/ridl-backend-rust/src/tests.rs:668,2365`
- Modify: `crates/ridl-backend-ts/src/interact.rs:312,406,416`
- Modify: `crates/ridl-backend-ts/src/interact/tests.rs` (local helper
  `final_def`)
- Modify: `crates/ridl-lsp/src/hover.rs:519,577`
- Modify: `crates/ridl-diff/src/walk.rs:636,884`

**Interfaces:**

- Consumes: nothing — this is the first task.
- Produces: `v2::FixedDef { payload: Option<FieldType> }` and the oneof variant
  `v2::decl::Kind::FixedDef(FixedDef)`. Task 2 depends on both names.

- [ ] **Step 1: Rename the message in the proto schema**

In `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`, line 103 inside the `kind` oneof:

```proto
FixedDef fixed_def = 20;
```

and the message at line 488:

```proto
message FixedDef {
  // Named type or array (ridl §8).
  FieldType payload = 1;
}
```

Also update the schema header comment at line 9, which lists the five kinds:

```proto
// reference §3–§14): interfaces holding the five interaction kinds —
// signal, event, command, query, fixed — plus services publishing them.
```

- [ ] **Step 2: Run the build to see every consumer break**

Run: `cargo build --workspace --locked 2>&1 | grep -E "^error" | head -20`
Expected: FAIL with `cannot find type FinalDef` / `no variant named FinalDef`
errors in `ridl-sem`, both backends, `ridl-lsp`, and `ridl-diff`. This error
list is the authoritative work list for step 3.

- [ ] **Step 3: Update each consumer**

`crates/ridl-sem/src/check.rs:2943`:

```rust
ast::InterfaceMember::Final(fin) => v2::decl::Kind::FixedDef(self.lower_final(fin)),
```

`crates/ridl-sem/src/check.rs:3909` — the return type only; the function keeps
its `lower_final` name until task 2:

```rust
fn lower_final(&mut self, fin: &ast::FinalDef) -> v2::FixedDef {
```

and its tail at line 3951:

```rust
v2::FixedDef { payload }
```

`crates/ridl-backend-rust/src/interact.rs:554`:

```rust
Some(v2::decl::Kind::FixedDef(fixed_def)) => {
```

`crates/ridl-backend-ts/src/interact.rs:406,416`:

```rust
Some(v2::decl::Kind::FixedDef(fixed_def)) if face == Face::Consumer => {
```

```rust
Some(v2::decl::Kind::FixedDef(_)) => Ok(String::new()),
```

`crates/ridl-lsp/src/hover.rs:519,577` and
`crates/ridl-diff/src/walk.rs:636,884` take the same substitution:
`Kind::FinalDef` becomes `Kind::FixedDef`, and the bound variable `final_def`
becomes `fixed_def`. In the two test modules
(`crates/ridl-backend-rust/src/tests.rs`,
`crates/ridl-backend-ts/src/interact/tests.rs`) rename the struct literal
`v2::FinalDef { .. }` to `v2::FixedDef { .. }` and the local helper function
`final_def` to `fixed_def`.

Leave every string literal and doc comment containing the word "final" alone in
this task — those belong to task 2.

- [ ] **Step 4: Compile**

Run: `just compile` Expected: PASS.

- [ ] **Step 5: Run the tests and inspect the snapshot diff**

Run: `just test` Expected: FAIL — the IR JSON snapshots hold the oneof variant
name. The failing files are
`crates/ridlc/tests/snapshots/corpus__ir@veh-cluster.snap`,
`corpus__ir@services-workspace.snap`, `corpus__ir@veh-common.snap`,
`corpus__ir@workspace-two-members.snap`, and
`crates/ridl-sem/src/snapshots/ridl_sem__check__tests__appendix_a_ir.snap`.

- [ ] **Step 6: Review the pending snapshots before accepting**

Run: `cargo insta pending-snapshots --as-json | head -40` then
`git diff --no-index` on any pair you want to read in full, or step through with
`cargo insta review`. Expected: every difference is `"FinalDef"` becoming
`"FixedDef"`. If any other key or value moved, stop and investigate — that is a
real change, not churn.

- [ ] **Step 7: Accept the snapshots and re-run**

Run: `cargo insta accept && just test` Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/ridl-ir crates/ridl-sem crates/ridl-backend-rust \
        crates/ridl-backend-ts crates/ridl-lsp crates/ridl-diff crates/ridlc
git commit -m "refactor(ir): rename the IR FinalDef message to FixedDef

Rename the IR v2 message and its oneof field, keeping field number 20
unchanged — the number is the compatibility contract under ADR-0008
decision 8, the name is not.

This is the schema half of the final to fixed keyword unification. The
source keyword is unchanged by this commit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Rename the surface keyword

Atomic by necessity: the lexer, the parser, every `.ridl` fixture, and the
compiled book examples must move together or the corpus stops parsing.

**Files:**

- Modify: `crates/ridl-syntax/family.ungram:189-193,204,219-220`
- Modify: `xtask/src/codegen.rs:91`
- Modify: `crates/ridl-syntax/src/keywords.rs:72,116`
- Modify: `crates/ridl-syntax/src/syntax_kind.rs:51,141`
- Modify: `crates/ridl-syntax/src/ast.rs:228,239,251,377,401`
- Modify: `crates/ridl-syntax/src/ast/generated.rs` (regenerated, never by hand)
- Modify: `crates/ridl-syntax/src/parser.rs:40-41,219,811,982,989`
- Modify: `crates/ridl-sem/src/check.rs` — 852, 2745-2758, 2943, 3879-3951,
  4132, 4160-4163, 4490, 4500, 4530, 5810. Line 2943 was already touched by task
  1 on its right-hand side; this task renames the left-hand side pattern
  `ast::InterfaceMember::Final(fin)` to `ast::InterfaceMember::Fixed(fin)`
- Modify: `crates/ridl-core/src/diag.rs:465,474,545,548`
- Modify: `crates/ridl-lsp/src/complete.rs:45,134`
- Modify: `crates/ridl-lsp/src/hover.rs:578`
- Modify: `crates/ridl-diff/src/walk.rs:862` — task 1 already moved this line's
  `Kind::FinalDef` pattern; only its `"final"` string literal remains
- Modify: `crates/ridl-diff/src/classify.rs:817,821`
- Modify: `crates/ridl-diff/src/lib.rs:141`
- Modify: `crates/ridl-backend-rust/src/interact.rs:15,341,565`
- Modify: `crates/ridl-backend-ts/src/interact.rs:17,385-403,447`
- Test data: `crates/ridl-syntax/test_data/lexer/interactions.ridl`,
  `crates/ridl-syntax/test_data/lexer/reserved_words.typl:4`,
  `crates/ridl-syntax/test_data/parser/ok/appendix_a_full_example.ridl`,
  `crates/ridl-syntax/test_data/parser/ok/lenient_overapprox.ridl`
- Rename: `crates/ridl-syntax/test_data/parser/ok/finals_reserved.ridl` to
  `fixeds_reserved.ridl`, and its snapshot alongside
- Test data, the eight corpus files: `ridl-diag-showcase/main/exposure.ridl`,
  `ridl-diag-showcase/main/kinds.ridl`,
  `ridl-diag-showcase/main/narrowing.ridl`,
  `services-workspace/contracts/shapes.ridl`,
  `services-workspace/vehicle/publish.ridl`,
  `veh-cluster/cluster/appendix-a.ridl`,
  `veh-cluster/cluster/internal-shape.ridl`, `veh-cluster/cluster/services.ridl`
  — all under `crates/ridlc/tests/corpus/`
- Test data, the five malformed files under `crates/ridlc/tests/malformed/`:
  `bare_keywords.ridl`, `nameless_inline_service.ridl`,
  `nameless_interactions.ridl`, `reserved_word_names.ridl`,
  `truncated_stream.ridl`. Read `reserved_word_names.ridl` and
  `bare_keywords.ridl` carefully rather than substituting blindly: they assert
  what happens when a reserved word appears in identifier position, so which
  word they should name after the registry change is a judgment call, not a
  substitution
- Modify: `docs/book/getting-started.md` (32 fenced and prose sites),
  `docs/book/cli-reference.md:718`, `docs/book/introduction.md:13`

**Interfaces:**

- Consumes: `v2::FixedDef` and `v2::decl::Kind::FixedDef` from task 1.
- Produces: `SyntaxKind::FixedKw`, `SyntaxKind::FixedDef`, `ast::FixedDef`,
  `ast::InterfaceMember::Fixed`, and `MemberKind::Fixed`. No later task consumes
  these.

- [ ] **Step 1: Write the failing lexer test**

Append to the test module in `crates/ridl-syntax/src/keywords.rs`:

```rust
#[test]
fn fixed_is_an_active_ridl_keyword_and_final_is_not_reserved() {
    assert_eq!(
        keyword_kind("fixed", Profile::Ridl),
        Some(SyntaxKind::FixedKw),
        "`fixed` is the provisioned-constant keyword in the ridl profile"
    );
    assert!(
        !FAMILY_RESERVED.contains(&"final"),
        "`final` was retired from the registry, the way `default` was"
    );
}
```

If the private helper is not named `keyword_kind`, use whichever function the
neighbouring `used_keywords_map_to_their_kind` test at line 268 calls.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p ridl-syntax fixed_is_an_active_ridl_keyword -- --nocapture`
Expected: FAIL to compile with
`no variant named FixedKw found for enum SyntaxKind`.

- [ ] **Step 3: Update the grammar and regenerate the AST**

In `crates/ridl-syntax/family.ungram`, keep the rule in its existing position so
the generated ordering does not churn. Line 204 inside `InterfaceMember`:

```
| FixedDef
```

and lines 219-220:

```
FixedDef =
  'fixed' Name ':' payload:FieldType Timing? AttrBlock?
```

The comment at lines 189-193 names the kinds an over-approximating parse
accepts; substitute `fixed` for `final` in all four mentions.

In `xtask/src/codegen.rs:91`:

```rust
"fixed" => ("FixedKw", "fixed"),
```

Then regenerate — never hand-edit `generated.rs`:

Run: `cargo xtask codegen` Expected:
`wrote crates/ridl-syntax/src/ast/generated.rs`.

- [ ] **Step 4: Update the keyword registry**

In `crates/ridl-syntax/src/keywords.rs`, line 72 inside `RIDL_KEYWORDS`:

```rust
("fixed", SyntaxKind::FixedKw),
```

In `FAMILY_RESERVED`, **delete** line 116 (`"final",` under the `// ridl.`
comment). Do not delete `"fixed"` at line 128 under `// uxdl.` — instead move
it: `fixed` is now shared, so put the single entry under the `// ridl.` group
where line 116 was, and leave a comment recording that uxdl uses the same word:

```rust
"query",
// Shared by ridl and uxdl — one registry entry per concept (typl §1.4).
"fixed",
// uxdl.
"view",
```

- [ ] **Step 5: Update the syntax kinds, AST wrapper, and parser**

`crates/ridl-syntax/src/syntax_kind.rs:51` becomes `FixedKw,` and line 141
becomes `FixedDef,`. Keep both in place so the `u16` discriminants do not shift.

`crates/ridl-syntax/src/ast.rs`: line 228 `Fixed(FixedDef),`, line 239
`SyntaxKind::FixedDef => FixedDef::cast(syntax).map(Self::Fixed),`, line 251
`Self::Fixed(it) => it.syntax(),`, and the two trait impls at 377 and 401 for
`FixedDef`.

`crates/ridl-syntax/src/parser.rs`: line 219 `| SyntaxKind::FixedKw`, line 811
`Some(SyntaxKind::FixedKw) => self.value_interaction(SyntaxKind::FixedDef),`,
the doc comment at 982 naming `FixedDef`, and the bump comment at 989
`// 'signal' | 'event' | 'fixed'`. The comment block at lines 40-41 names the
kinds an over-approximating parse accepts — substitute there too. Leave line 262
alone; "a final entry for the end" means "last".

- [ ] **Step 6: Update the checker, its messages, and the diagnostic summaries**

In `crates/ridl-sem/src/check.rs`: rename `lower_final` to `lower_fixed` and its
parameter type to `&ast::FixedDef`; rename `MemberKind::Final` (line 4490) to
`MemberKind::Fixed` and its display arm (line 4500) to `Self::Fixed => "fixed"`.
The message strings become:

```rust
"a `fixed` is provisioned externally and never republished, so it has no rate \
```

```rust
"fixed payload must be a named type or an array".to_string(),
```

```rust
"init value not valid on fixed".to_string(),
```

```rust
self.reject_timing(&timing, "fixed");
```

```rust
"an attribute block is not valid on `fixed` — a `fixed` has no timing to \
```

```rust
"stream `<T>` not valid on fixed".to_string(),
```

The comment block at 2745-2758 and the doc comments at 852, 4132, 4530, and 5810
name the kinds in prose — substitute there.

In `crates/ridl-core/src/diag.rs`, the RIDL-106 summary at 465 and 474 and the
RIDL-301 summary at 545 and 548:

```rust
"timing annotation on a kind that carries none, or attribute block on `fixed`";
```

```rust
"`require` or `ensure` on `signal`, `event`, or `fixed`";
```

- [ ] **Step 7: Update the LSP, the diff labels, and the backend doc comments**

`crates/ridl-lsp/src/complete.rs:45`:

```rust
const INTERACTION_KEYWORDS: &[&str] = &["signal", "event", "command", "query", "fixed", "reserved"];
```

and line 134 `| SyntaxKind::FixedKw`. `crates/ridl-lsp/src/hover.rs:578` becomes
`"fixed {name} : {}"`. Leave lines 324 and 334 of `complete.rs` and every "final
segment" in `nav.rs` and `rename.rs` untouched.

`crates/ridl-diff/src/walk.rs:862` becomes
`Some(Kind::FixedDef(_)) => "fixed",`. `classify.rs:817,821` and `lib.rs:141`
name the kinds in explain text and doc comments — substitute.

In both backends, the doc comments and the emitted doc text at
`crates/ridl-backend-rust/src/interact.rs:15,341,565` and
`crates/ridl-backend-ts/src/interact.rs:17,385-403,447` name the kind in prose
and in generated output. The emitted line at `interact.rs:565` becomes:

```rust
"fixed `{source_name}` — ordinal {ordinal} (ridl §8).\n\
```

Leave `crates/ridl-backend-rust/src/lib.rs:55` ("as a final guard") alone.

- [ ] **Step 8: Update every `.ridl` and `.typl` fixture**

Substitute the declaration keyword in the four `ridl-syntax` test-data files,
the eight `ridlc` corpus files, and the four `ridlc` malformed files listed
under **Files**. In `crates/ridl-syntax/test_data/lexer/reserved_words.typl`,
**delete** `final` from line 4 — it is no longer a registry word, so it would
now lex as an ordinary identifier and the fixture would be asserting the wrong
thing:

```
interface service signal event command query
```

Rename the parser fixture and its snapshot:

```bash
git mv crates/ridl-syntax/test_data/parser/ok/finals_reserved.ridl \
       crates/ridl-syntax/test_data/parser/ok/fixeds_reserved.ridl
git mv "crates/ridl-syntax/tests/snapshots/parser_corpus__ridl_ok_corpus_is_lossless_error_free_and_matches_snapshots@finals_reserved.ridl.snap" \
       "crates/ridl-syntax/tests/snapshots/parser_corpus__ridl_ok_corpus_is_lossless_error_free_and_matches_snapshots@fixeds_reserved.ridl.snap"
```

- [ ] **Step 9: Update the book**

`docs/book/getting-started.md` holds 32 sites: the "Provisioned values — final"
heading at line 442, the prose at 200-201 and 464-467, the naming guidance at
852, and the fenced `ridl` examples. Every fence in that file is compiled by
`crates/ridl/tests/book_examples.rs`, so a missed one fails the gate rather than
shipping. Also `docs/book/cli-reference.md:718` ("A signal, event, or final
payload type changed.") and `docs/book/introduction.md:13` (the ridl row of the
family table).

Then reformat:

Run: `just fmt` Expected: no error.

- [ ] **Step 10: Run the failing test from step 1, then the suite**

Run: `cargo test -p ridl-syntax fixed_is_an_active_ridl_keyword` Expected: PASS.

Run: `just test` Expected: FAIL — the parser, backend, and diagnostic snapshots
hold the old keyword. Review with `cargo insta review`, confirming that every
difference is a `final` to `fixed` substitution and nothing else, then
`cargo insta accept`.

- [ ] **Step 11: Run the whole gate**

Run: `just build` Expected: PASS. This covers `fmt-check`, the compiled book
examples via `book-check` and `test`, clippy, the wasm target, and the Markdown
lint.

- [ ] **Step 12: Verify no keyword occurrence survives**

Run:

```bash
grep -rniE "\bfinal\b" crates/ xtask/ docs/book/ | grep -viE "final segment|final guard|final entry"
```

Expected: no output. Any line that appears is either a missed site or a new
ordinary-English use that belongs in the exclusion list above — decide which
before continuing.

- [ ] **Step 13: Commit**

```bash
git add -A
git commit -m "feat(ridl-syntax)!: rename the ridl final keyword to fixed

Rename the provisioned-constant keyword so ridl and uxdl use one word for
one concept. A Java or Kotlin reader takes final for a compile-time
constant; the primitive is provisioned externally and immutable for the
lifetime of one software instance.

Remove final from the family reserved-word registry, following the
precedent typl set when it retired default without reserving it. The
registry entry for fixed moves to the shared group.

Diagnostic codes RIDL-106, RIDL-301, and FORM-102 are unchanged; only
their message text moves.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Update the VS Code grammar

No gate covers this file, so it is the one that silently rots.

**Files:**

- Modify: `editors/vscode/syntaxes/ridl.tmLanguage.json:10,76`

**Interfaces:**

- Consumes: the keyword decided in task 2.
- Produces: nothing consumed downstream.

- [ ] **Step 1: Update the keyword pattern and the header comment**

Line 76:

```json
"match": "\\b(signal|event|command|query|fixed)\\b",
```

Line 10, the header comment listing what the grammar highlights:

```json
"(interface service signal event command query fixed require ensure), the `@`",
```

- [ ] **Step 2: Verify the JSON still parses**

Run:
`python3 -m json.tool editors/vscode/syntaxes/ridl.tmLanguage.json > /dev/null && echo ok`
Expected: `ok`.

- [ ] **Step 3: Verify no other keyword site remains in the extension**

Run: `grep -rn "final" editors/vscode/` Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add editors/vscode
git commit -m "fix(editors): highlight fixed instead of final

The TextMate grammar carries its own copy of the interaction keyword
list, and no gate checks it against the lexer.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Update the language references and close the open question

**Files:**

- Modify: `docs/specification/ridl-language-reference.md` — 21 sites, including
  §8 and its heading (457, 463-465), the five-kinds table (169), the glossary
  (1416), the RIDL-106 and RIDL-301 rows (1010, 1040), Appendix B (1229), and
  Appendix C (1265, 1275, 1285)
- Modify:
  `docs/specification/typl-language-reference.md:76,108,110,142,1580,1654`
- Modify: `docs/specification/uxdl-language-reference.md:77,285,448,600,618,693`
- Modify: `docs/specification/ridl-family-overview.md:140`
- Modify: `docs/specification/README.md:12`
- Modify: `docs/technotes/walking-skeleton-architecture.md:44`
- Modify: `docs/wip/family-general-form.md:55,290,349,464-469,527,547`
- Modify: `docs/ROADMAP.md:124,140`

**Interfaces:**

- Consumes: the keyword decided in task 2.
- Produces: nothing consumed downstream.

- [ ] **Step 1: Rewrite ridl §8 and its supporting tables**

The §8 heading becomes `## 8. Fixed`, the table of contents entry at line 38
becomes `8. [Fixed](#8-fixed)`, and the kinds table row at 169 becomes:

```markdown
| `fixed`   | provisioned | immutable for the software-instance lifetime       | neither (provisioned) |
```

The naming note inside §8 (line 473) currently records "`final`, not `config`".
Replace it with a note that records the current state:

```markdown
- Naming decision on record: `fixed`, not `final` and not `config` — one word
  for one concept across ridl and uxdl (ADR-0011); `config` connotes hot-reload
  and is reserved vocabulary space for rsdl
```

In Appendix C, line 1275 becomes
`fixed_def     = doc_comment? "fixed"   camelCase_id ":" fixed_type ;` and line
1285 becomes `fixed_type    = type_ref | array_type ;`.

- [ ] **Step 2: Fix the uxdl grammar dangle**

`docs/specification/uxdl-language-reference.md:618` currently references
`final_type`, which no uxdl production defines. It becomes:

```ebnf
fixed_def     = doc_comment? "fixed" camelCase_id ":" fixed_type ;
```

and `fixed_type` joins the borrowed-production list in the appendix preamble at
line 600, which today reads `timing`, `init_value`, `attr_block`, `reserved`,
`param_list`, `stream_type`, `return_type`.

- [ ] **Step 3: Update the interact core table and uxdl §8**

The table row at `uxdl-language-reference.md:77` keeps its place, with both
cells reading `fixed` and a note that this is the one primitive both profiles
spell the same:

```markdown
| provisioned constant   | `fixed`                | `fixed` (the one shared spelling) |
```

§8 at line 285 becomes "A **static capability** — the uxdl profile of `fixed`,
the same keyword ridl uses. All of ridl §8 applies." Line 448's UXDL-105 row
lists the system-interaction keywords a `.uxdl` file rejects; `fixed` is no
longer one of them, so remove it from that list and leave `signal`, `event`,
`command`, `query`, `interface`.

- [ ] **Step 4: Update typl's registry and mapping tables**

Line 76's rejected-in-`.typl` column, line 108's interaction-keyword sentence,
line 110's `config` mapping, and line 142's family-wide reserved list all name
`final`. In line 142, `fixed` must move out of the uxdl group into a shared
position, matching the code change in task 2 step 4. Lines 1580 and 1654 are
Appendix F prior-art rows naming ridl `final`.

- [ ] **Step 5: Close the open question**

`docs/wip/family-general-form.md` §6.5 (lines 464-469) is the reopened naming
question. Replace its body with the decision and a pointer to ADR-0011, and
change the summary-table row at 527 from "**Reopened**, undecided (§6.5)" to
"**Decided** — `fixed` (ADR-0011)". Remove item 6 from the open-question list at
line 547. Update the shape-1 table at 55 and the examples at 66, 290, and 349.

`docs/specification/ridl-family-overview.md:140` is decision-ledger item 6,
which today reads "`final` over `config` (naming ledger)". It becomes:

```markdown
| 6  | `fixed` over `final` over `config` — one word for one concept at both boundaries | ADR-0011, concept note §10 |
```

`docs/ROADMAP.md:124` lists the reconsideration as an open ADR-0008 item —
remove that clause. Line 140 names the five kinds in the E2.1 row.

- [ ] **Step 6: Reword the prose that uses "fixed" as an ordinary adjective**

Now that `fixed` is a keyword in two profiles, prose using it as an English
adjective for array and buffer sizes reads ambiguously. Reword these six sites
to "exact-length" or "exactly N":

- `docs/specification/typl-language-reference.md:455,457` — "fixed N
  characters", "fixed N with validation"
- `docs/specification/typl-language-reference.md:466` — "fixed N bytes"
- `docs/specification/typl-language-reference.md:884` — the "Fixed array" row
  label
- `docs/book/getting-started.md:612,613` — the two trailing comments inside a
  compiled fence, "fixed array — exactly 8" and "fixed 64-byte buffer". They are
  comments, so the fence still compiles either way, but they sit two lines apart
  from real declarations and are the most confusing instance in the book

Leave `docs/archive/` alone — it is verbatim history.

- [ ] **Step 7: Format and lint**

Run: `just fmt && just check` Expected: PASS.

- [ ] **Step 8: Verify no live document still names the keyword**

Run:

```bash
grep -rniE "\bfinal\b" docs/specification/ docs/book/ docs/wip/ docs/technotes/ docs/ROADMAP.md
```

Expected: no output. `docs/archive/` and `docs/decisions/` are deliberately
outside this sweep.

- [ ] **Step 9: Commit**

```bash
git add docs/
git commit -m "docs(docs): rename final to fixed across the language references

Update the five references, the overview ledger, the general form, the
roadmap, and the walking-skeleton technote for the keyword decided in
ADR-0011.

Also fix a dangling nonterminal found while surveying the question: uxdl
Appendix C referenced final_type, which only ridl's appendix defined and
which uxdl's borrowed-production list did not name.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Record ADR-0011 and supersede ADR-0008 decision 5

**Files:**

- Create: `docs/decisions/ADR-0011-provisioned-constant-keyword.md`
- Modify: `docs/decisions/ADR-0008-e2-execution.md:448-450` and wherever the
  whole-document sweep finds a sentence this change falsifies
- Modify: `docs/decisions/README.md` (the ADR index)

**Interfaces:**

- Consumes: the decision as implemented in tasks 1 to 4.
- Produces: the durable record every later epic cites.

- [ ] **Step 1: Write ADR-0011**

Follow the ADR-0009 and ADR-0010 shape: `## Status` (Accepted, dated 2026-07-27,
explicitly not epic-scoped and binding until superseded), `##
Context`, numbered
`## Decision` entries, and `## Consequences`. The content is the design document
at `docs/wip/2026-07-27-fixed-keyword-unification-design.md` — carry over §3.1's
argument (uxdl §8 is the only section that delegates wholesale, so it is the
only row that does not need profile-specific words), §3.2's rejected-candidate
table verbatim, and the two sub-decisions from §4.1 and §4.5.

State the decisions as at least these five, numbered:

1. ridl's `final` becomes `fixed`; uxdl is unchanged.
2. `final` is removed from the reserved-word registry, following typl's
   `default` precedent — no alias and no migration diagnostic.
3. IR field number 20 is unchanged; only the message and field names move.
4. Diagnostic codes are unchanged; only message text moves.
5. The interact-core table keeps the provisioned-constant row with both cells
   reading `fixed`.

- [ ] **Step 2: Add the superseding pointer to ADR-0008**

Decision 5 at lines 448-450 keeps its text — it is an accurate record of what
was decided for E2. Append one sentence:

```markdown
_Superseded (2026-07-27) by ADR-0011: E2 is closed, and the general form's
§6.5 question is decided — the keyword is `fixed`._
```

- [ ] **Step 3: Run the whole-document sweep ADR-0008 demands**

That document's editing note requires sweeping the entire file, against the file
and not against the diff, for sentences this edit has falsified. Read
`docs/decisions/ADR-0008-e2-execution.md` start to finish and check every
mention of `final` (lines 373, 390, 448-450, 1282, 1300 among them) plus the
`## Status` line and `## Consequences`. A sentence that describes what E2 did
stays as it is; a sentence that asserts the keyword's present or future state
must gain the supersession. Record the sweep's result in the commit message,
including the count of sentences corrected, the way that document's earlier
sweeps did.

- [ ] **Step 4: Add ADR-0011 to the index**

Add the row to `docs/decisions/README.md` in the existing table format, and add
it to the ADR list in `AGENTS.md` under "Read these before doing anything else",
which today enumerates ADR-0002 through ADR-0009 and needs ADR-0010 and
ADR-0011.

- [ ] **Step 5: Format, lint, verify**

Run: `just fmt && just check` Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/decisions AGENTS.md
git commit -m "docs(adr): record ADR-0011, the provisioned-constant keyword

Record the decision to spell the provisioned constant fixed in both ridl
and uxdl, with the rejected candidates and the reasoning that only this
interact-core row is a candidate for one shared spelling.

Supersede ADR-0008 decision 5, which froze final for the duration of E2.
The whole-document sweep that ADR's editing note requires was run against
the file.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Garden the working memory and open the PR

**Files:**

- Move: `docs/wip/2026-07-27-fixed-keyword-unification-design.md` to
  `docs/archive/`
- Move: `docs/wip/2026-07-27-fixed-keyword-unification-plan.md` to
  `docs/archive/`

**Interfaces:**

- Consumes: ADR-0011 from task 5, which is the durable record these two
  transient documents were gardened into.
- Produces: an empty `docs/wip/` for this work, which is the merge precondition.

- [ ] **Step 1: Confirm the durable record exists before archiving**

Run:
`test -f docs/decisions/ADR-0011-provisioned-constant-keyword.md && echo ok`
Expected: `ok`. If this fails, task 5 is incomplete and archiving would lose the
only copy of the reasoning.

- [ ] **Step 2: Archive both documents verbatim**

```bash
git mv docs/wip/2026-07-27-fixed-keyword-unification-design.md docs/archive/
git mv docs/wip/2026-07-27-fixed-keyword-unification-plan.md docs/archive/
```

`docs/wip/family-general-form.md`, `ridl-family-concept.md`,
`skill-ridl-authoring-outline.md`, and `README.md` stay — they are long-lived
working specs, not this session's working memory.

- [ ] **Step 3: Run the full pre-PR gate**

Run: `just verify` Expected: PASS — this runs `lint-commits` over the branch and
then the whole `build` gate.

- [ ] **Step 4: Commit and open the PR**

```bash
git add -A
git commit -m "docs(docs): archive the fixed-keyword working memory

The design and plan are gardened into ADR-0011; the originals move to
docs/archive/ verbatim.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

Open the PR against `main` with a body that states the keyword change, the
absence of any external consumer (version `0.0.0`, no tags, `publish = false`),
and the fact that the `.ir.json` variant key moved from `"FinalDef"` to
`"FixedDef"`.

---

## Verification Summary

| Task | Gate                                                         |
| ---- | ------------------------------------------------------------ |
| 1    | `just compile`, `just test`, snapshot diff reviewed by eye   |
| 2    | `just build`, plus the grep in step 12 returning no output   |
| 3    | JSON parses, `grep -rn "final" editors/vscode/` returns none |
| 4    | `just check`, plus the scoped document grep in step 8        |
| 5    | `just check`, plus the ADR-0008 whole-document sweep         |
| 6    | `just verify`, plus `docs/wip/` holding no session artifacts |
