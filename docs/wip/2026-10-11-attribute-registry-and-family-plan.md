# Attribute registry, fail-closed diff, and the interaction family — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** settle the language surface that roadmap stories E3.1 to E3.3 touch
before lane L switches on the `keywords` and `diagnostics` freeze rows: free the
retired uxdl words, make one attribute table the only allow-list the checkers
read and the only source of what `ridl diff` reports, and add `family` and
`shape` to the IR with their complete value sets.

**Architecture:** three pull requests, each cut from `origin/main` on its own
branch, in order. PR 1 edits the reserved-word registry in `ridl-syntax` and the
two references that state it. PR 2 adds `ridl_core::attributes`, makes the ridl
checker and the rsdl attribute reader look it up, lowers `tier` and `deprecated`
into the system artifact, and makes the system diff report every `Compared` row.
PR 3 adds the `Family` and `Shape` enums to `ir.proto` and `model.proto` with
zero values chosen so that no encoded byte changes, a `check_interaction_class`
function that the IR readers and writers call, and the `InteractionClassChanged`
diff category.

**Tech Stack:** Rust 2024 (the pin in `rust-toolchain.toml`), prost and
pbjson-build for the IR, the `xtask freeze` dumps, prim for Markdown.

**Spec:** `docs/wip/2026-10-11-attribute-registry-and-family-design.md`.
Decisions D-1 to D-6 of its section 2 are settled; this plan does not reopen
them.

## Global Constraints

- Three branches, in this order, each cut from `origin/main`:
  `feat/reserved-words-370` (PR 1, spec section 3), `feat/attribute-registry`
  (PR 2, spec section 4), `feat/interaction-family` (PR 3, spec section 5). Each
  pull request rebases onto `main` and runs `cargo xtask freeze update` before
  it merges (ADR-0029 decision 3; `.cargo/config.toml` aliases `cargo xtask` to
  `cargo run --package xtask --`).
- Gate: `just build` passes before each pull request is opened. Per task:
  `cargo test -p <crate> --locked`,
  `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --all --check`, and `cargo xtask freeze check` when a dump changes
  (regenerate with `cargo xtask freeze update`; the subcommand names are
  `update` and `check`, read in `xtask/src/freeze.rs`).
- No story id (`E3.1`, `epic E3`) and no plan name in code, rustdoc,
  `docs/book`, `docs/design` or `docs/technotes` (`just story-id-check`). Link
  an issue as `driftsys/ridl#N` instead.
- Prose — comments, rustdoc, docs, commit messages — is plain literal English,
  with no idiom and no figure of speech.
- Conventional Commits with a scope from `.git-std.toml`: the scopes this plan
  uses are `ridl-syntax`, `ridl-core`, `ridl-sem`, `ridl-ir`, `ridl-diff`,
  `ridlc`, `ridl`, `xtask`, `typl`, `ridl`, `rsdl`, `adr`, `docs`. Every commit
  message ends with the line
  `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`.
- TDD with a semantic red: every failing test fails on an assertion, not on a
  compile error. Where a test needs a name that does not exist yet, the task
  adds a stub first (an empty table, a function that returns `Ok(())`), so the
  red is the assertion.
- No diagnostic code is added, removed or renamed. `freeze/diagnostics.txt` is
  expected unchanged; Tasks 1, 5 and 8 check it with
  `git diff --exit-code freeze/diagnostics.txt`.
- The catalog hash of `examples/cabin` and `freeze/wire.txt` are byte-identical
  after PR 3. Task 8 adds the test that pins it; `cargo xtask freeze check`
  holds `freeze/wire.txt` to its committed text.
- Every `ridl`/`typl`/`rsdl` fence in `docs/book/` is compiled by
  `crates/ridl/tests/book_examples.rs`. No task adds or edits such a fence; a
  task that touches a `docs/book/` file runs that test.
- Existing struct literals of `v2::SignalDef`, `v2::EventDef`, `v2::CommandDef`,
  `v2::QueryDef`, `v1::Interaction` and `v1::CommandShape` gain the new fields
  in PR 3 as explicit values (`family: v2::Family::Dispatch as i32`), never as
  `..Default::default()` on a literal that today lists every field, so that the
  next added field is still a compile error at each site.

## Review Focus

Inputs the spec implies and no task's tests exercise directly. Each line names
its test, added to the task that owns the code.

1. A source that uses a freed word as an identifier in every position — a
   package segment, a type name, a field, an interaction name, a parameter —
   must compile in every profile. Task 1 adds
   `freed_words_are_identifiers_in_a_ridl_source` over such a source.
2. A backend key whose namespace is the name of a core key (`labels.x`,
   `tier.y`) must stay a backend key (FORM-106 only when the namespace is not
   camelCase, RSDL-804 when unclaimed), never be read as the core key. Task 5
   adds `a_namespace_spelled_like_a_core_key_is_a_backend_key` to the rsdl
   attribute test table.
3. A `Compared` row's value that moves from absent to present (a `deprecated`
   added where none was) must be reported, not only a change between two values.
   Task 6's `the_registry_rows_are_reported_as_recorded` mutates from the base,
   which holds none, so the absent-to-present direction is what it tests; Task 6
   adds the removal direction in `an_attribute_removed_is_reported`.
4. An IR snapshot written by 0.9, with no `family` key in its JSON, must read as
   `FAMILY_DISPATCH` and pass `check_interaction_class`. Task 9 adds
   `a_snapshot_without_family_reads_as_dispatch`.
5. A codegen request that carries `(command, intent, none)` must be refused by
   `request_from_json`, the lenient reader, exactly as the strict reader refuses
   it. Task 9 adds `the_lenient_request_reader_rejects_an_invalid_class`.

---

## PR 1 — reserved words (driftsys/ridl#370)

Branch `feat/reserved-words-370`, cut from `origin/main`. Spec section 3. Not
breaking: removing a reserved word cannot invalidate a source. Before the pull
request merges, rebase onto `main` and run `cargo xtask freeze update`; only
`freeze/keywords.txt` may change. `freeze/surfaces.toml` holds no `keywords` row
today, so the check compares the dump with its committed text only; the row is
switched on after PR 3 (spec section 6).

### Task 1: free the retired uxdl words in the registry

**Files:**

- Modify: `crates/ridl-syntax/src/keywords.rs` (the `FAMILY_RESERVED` table, its
  doc comment, the `tests` module)
- Modify: `crates/ridl-syntax/src/lexer.rs:476` (the test
  `other_profile_words_stay_reserved_in_both_profiles`)
- Modify: `crates/ridl-syntax/src/parser.rs:614` (one rustdoc line that names
  "user-interaction" words of the uxdl profile)
- Modify: `crates/ridl-sem/src/check.rs` (one new test in the `tests` module)
- Regenerate: `freeze/keywords.txt`

**Interfaces:**

- Consumes:
  `ridl_syntax::keywords::{FAMILY_RESERVED, is_reserved, keyword_in, Profile}`
  as they are.
- Produces: `FAMILY_RESERVED` without the 16 words. No signature changes;
  `ridl init` (`crates/ridl/src/scaffold.rs:207`), the LSP rename checks and the
  `keywords` dump read the table through `is_reserved` and `FAMILY_RESERVED` and
  need no edit.

- [ ] **Step 1: Write the failing registry test**

Append to the `tests` module of `crates/ridl-syntax/src/keywords.rs`:

```rust
    #[test]
    fn the_retired_uxdl_words_are_identifiers() {
        // The retired uxdl words leave the registry, except `states` and
        // `during`, which the general form names as ridl clauses
        // (ADR-0012 decision 4 as amended on 2026-10-11).
        for word in [
            "view", "display", "input", "action", "activate", "toggle", "select", "adjust",
            "dismiss", "fetch", "navigate", "scroll", "drag", "observe", "surface", "agent",
        ] {
            assert!(!is_reserved(word), "`{word}` must be an identifier");
            for profile in [Profile::Typl, Profile::Ridl, Profile::Rsdl] {
                assert_eq!(
                    keyword_in(profile, word),
                    None,
                    "`{word}` must not be a keyword under {profile:?}"
                );
            }
        }
        for word in ["states", "during"] {
            assert!(is_reserved(word), "`{word}` stays reserved: a ridl clause word");
        }
    }

    #[test]
    fn the_domain_spellings_are_identifiers_until_a_profile_implements_them() {
        // A spelling is a keyword only as the first token of an interface
        // member, in a profile that implements it; no profile does yet
        // (ADR-0012 decision 4 as amended on 2026-10-11).
        for word in [
            "present", "notify", "measure", "detect", "actuate", "trigger", "supply", "read",
        ] {
            assert!(!is_reserved(word), "`{word}` must be an identifier");
        }
    }
```

- [ ] **Step 2: Run the tests to see the first one fail on its assertion**

Run: `cargo test -p ridl-syntax --locked the_retired_uxdl_words_are_identifiers`

Expected: FAIL with `` `view` must be an identifier ``. The second test passes
already (none of those words is in the table today); it is a pin, kept so that a
later profile cannot reserve a spelling by mistake.

- [ ] **Step 3: Remove the 16 words from `FAMILY_RESERVED`**

In `crates/ridl-syntax/src/keywords.rs`, replace the block from the comment
`// Shared by ridl and uxdl — one registry entry per concept (typl §1.4).` to
the line `"agent",` inclusive with:

```rust
// ridl. `fixed` is family-neutral (ADR-0012 decision 4).
"fixed",
// The two ridl clause words the general form names (§4.8). The other
// retired uxdl words left the registry with driftsys/ridl#370 and are
// identifiers in every profile.
"states",
"during",
```

Update the module doc comment's first paragraph (lines 1 to 6) to read:

```rust
//! The RIDL family reserved-word registry (typl reference §1.4).
//!
//! The family maintains one reserved-word registry across every profile: every
//! word in it is reserved in every profile, including words a profile does not
//! accept. The ADR-0012 domain spellings are not in it: a spelling is a keyword
//! only as the first token of an interface member, in a profile that implements
//! it. This module is the single source of truth the lexer maps identifiers
//! against, and the source a governance test can read.
```

- [ ] **Step 4: Repair the lexer test that used `view`**

In `crates/ridl-syntax/src/lexer.rs`, in
`other_profile_words_stay_reserved_in_both_profiles`, change the word list
`["view", "model", "component", "automaton"]` to
`["states", "model", "component", "automaton"]`.

- [ ] **Step 5: Repair the parser rustdoc that names the uxdl profile**

In `crates/ridl-syntax/src/parser.rs`, replace the two lines

```rust
/// - in a `.ridl` parse, any `ReservedWord` — a behaviour,
///   user-interaction, or architecture word of the uxdl/rmdl/rsdl
///   profiles — draws **RIDL-403**;
```

with

```rust
/// - in a `.ridl` parse, any `ReservedWord` — a behaviour or architecture
///   word of the rmdl/rsdl profiles, or a ridl clause word — draws
///   **RIDL-403**;
```

- [ ] **Step 6: Add the whole-source test (Review Focus 1)**

In `crates/ridl-sem/src/check.rs`, next to
`form_106_107_108_attribute_allow_list`, add:

```rust
#[test]
fn freed_words_are_identifiers_in_a_ridl_source() {
    // driftsys/ridl#370: a word that left the registry is legal in every
    // identifier position of a ridl source.
    let checked = check_ridl(
        "app",
        &format!(
            "{PRELUDE}struct Surface {{\n  view: Speed\n  agent: Speed\n}}\n\
             interface Display {{\n  signal input : Speed @10ms\n  \
             command navigate(scroll: Speed, drag: Speed) @[..50ms]\n  \
             query observe(): Speed @[..50ms]\n  event action : Surface @[100ms..1s]\n}}\n"
        ),
    );
    assert_eq!(codes(&checked), Vec::<&str>::new());
}
```

`PRELUDE`, `check_ridl` and `codes` are the helpers the neighbouring tests use;
`Speed` is declared by `PRELUDE`. If `PRELUDE` does not declare a type named
`Speed`, use the scalar type it does declare.

- [ ] **Step 7: Run the crate tests**

Run: `cargo test -p ridl-syntax --locked` and `cargo test -p ridl-sem --locked`

Expected: PASS. `every_used_keyword_lexes_to_its_variant` iterates
`FAMILY_RESERVED` and needs no edit.

- [ ] **Step 8: Regenerate the keywords dump and check the others**

Run: `cargo xtask freeze update` then `git status --short freeze/`

Expected: only `freeze/keywords.txt` changed. It loses the 16 `reserved <word>`
lines and its `reserved-set` line changes. Then run
`git diff --exit-code freeze/diagnostics.txt` (expected: exit 0) and
`cargo xtask freeze check` (expected: PASS).

- [ ] **Step 9: Run clippy and fmt, then commit**

Run: `cargo clippy -p ridl-syntax -p ridl-sem --all-targets -- -D warnings` and
`cargo fmt --all --check`.

```bash
git add crates/ridl-syntax/src/keywords.rs crates/ridl-syntax/src/lexer.rs \
  crates/ridl-syntax/src/parser.rs crates/ridl-sem/src/check.rs freeze/keywords.txt
git commit -m "feat(ridl-syntax): make the retired uxdl words identifiers

The 16 words ADR-0012 retired leave FAMILY_RESERVED. \`states\` and
\`during\` stay, because the general form names them as ridl clauses.
The ADR-0012 domain spellings were never in the table; a test now pins
that they stay out until a profile implements one.

Closes driftsys/ridl#370.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 2: state the member-start rule in the references and the book

**Files:**

- Modify: `docs/specification/typl-language-reference.md:132-158` (the "reserved
  family-wide" paragraph of §1.4)
- Modify: `docs/specification/ridl-language-reference.md:170-172` (the closing
  parenthetical of §2.3)
- Modify: `docs/book/getting-started.md:1045-1049`
- Modify: `docs/specification/ridl-family-overview.md` (§5 decision ledger, one
  row)

**Interfaces:**

- Consumes: the registry as Task 1 leaves it.
- Produces: the prose Task 3's amendment note cites (typl §1.4).

- [ ] **Step 1: Rewrite the "reserved family-wide" paragraph of typl §1.4**

Replace the paragraph that begins
`Keywords **reserved family-wide** but
rejected by`.typl`` and ends
`the union of those sections **is** the registry
until the platform spec extracts it as a standalone normative list.`
with these two paragraphs:

```markdown
Keywords **reserved family-wide** but rejected by `.typl` (the registry as
built, `crates/ridl-syntax/src/keywords.rs`): ridl's `interface`, `service`,
`signal`, `event`, `command`, `query` and `fixed` (family-neutral, ADR-0012
decision 4); the expr-core words `require`, `ensure`; the ridl clause words
`states` and `during` (general form §4.8); rmdl's `model`, `function`, `let`,
`init`, `last`, `case`, `if`, `then`, `else`, `when`, `emit` (it also surfaces
`signal`/`event` in signatures — same concepts, one registry entry each; `init`
is rmdl's alone — the memory-seed keyword, needed because a flow has two
equations; typl/ridl express init as bare `= value`; its ambient time values
`now`/`dt` are _contextual identifiers, not keywords_; `pre`,
`->`-as-followed-by, `node`, `returns`, `realizes`, and a surface `step`
considered and rejected — models are contract-blind, so no `realizes`; `step`
remains typl's quantization keyword alone), plus its reserved set `merge`,
`current`, `state`, `transition`, `automaton`; and rsdl's `system`, `component`,
`distribution`, `deployment`, `machine`, `offers`, `requires`, `for` (rsdl
reference §2 — the rsdl v0.1 words `provides`, `instance`, `assurance`,
`target`, `place`, `on`, `transport`, `bundle`, `time`, `base`, `redundant`,
`supervise` and `degraded` left the registry with rsdl v0.2, because no other
profile uses them, and each is an ordinary identifier in every profile). (`node`
and `returns` were considered and rejected by rmdl — never reserved.) The
per-profile keyword sections of each language reference enumerate their own
additions; the union of those sections **is** the registry until the platform
spec extracts it as a standalone normative list.

**The domain spellings are not reserved words — the member-start rule**
(ADR-0012 decision 4, as amended on 2026-10-11). `present`, `notify`,
`measure`, `detect`, `actuate`, `trigger`, the intent operation shapes
`activate`, `toggle`, `select`, `adjust`, `dismiss`, and the open candidates
`supply` and `read` are keywords only as the first token of an interface
member, in a profile that implements the spelling; no profile does in this
release. Everywhere else each is an identifier: the ROS 2 corpus names a field
`present` (`evals/corpus/ros2/sensor_msgs/messages.typl`) and a query `trigger`
(`evals/corpus/ros2/std_srvs/interactions.ridl`). An interface member line
never starts with an identifier, so a profile that gains a spelling invalidates
no source that compiled before. The uxdl words ADR-0012 retired — `view`,
`display`, `input`, `action`, `fetch`, `navigate`, `scroll`, `drag`, `observe`,
`surface`, `agent` — are ordinary identifiers in every profile.
```

- [ ] **Step 2: Point ridl §2.3 at the rule**

Replace the last parenthetical of §2.3,

```markdown
(`reserved` and the evolution machinery are inherited from typl §7.4.
`require`/`ensure` are expr-core words activated here. All family keywords
remain reserved in every profile — typl §1.4.)
```

with

```markdown
(`reserved` and the evolution machinery are inherited from typl §7.4.
`require`/`ensure` are expr-core words activated here. Every registry word
remains reserved in every profile, and the registry as built is the list in
typl §1.4. The ADR-0012 domain spellings — `present`, `trigger` and the rest —
are not in it: each is a keyword only as the first token of an interface member,
in a profile that implements it, and an identifier everywhere else — the
member-start rule, typl §1.4.)
```

- [ ] **Step 3: Repair the book's example list**

In `docs/book/getting-started.md`, replace

```markdown
Every family keyword is reserved in every profile, including keywords this
profile does not accept. `state`, `input`, `target`, `model`,
`component` and the rest of the registry are not available as identifiers even
in a `.ridl` file — the
typl language reference §1.4 lists the whole set.
```

with

```markdown
Every registry word is reserved in every profile, including words this profile
does not accept. `state`, `states`, `model`, `component` and the rest of the
registry are not available as identifiers even in a `.ridl` file — the typl
language reference §1.4 lists the whole set. The domain spellings of ADR-0012
(`present`, `trigger`, …) are not in the registry; they are identifiers until a
profile implements one.
```

(`input` left the registry in Task 1; `target` left it with rsdl v0.2, so the
old sentence was already stale on that word.)

- [ ] **Step 4: Add the ledger row to the overview**

In `docs/specification/ridl-family-overview.md` §5, after row 38, add:

```markdown
| 39 | **The domain spellings are not reserved words**: a spelling is a keyword only as the first token of an interface member, in a profile that implements it; the retired uxdl words are identifiers, `states` and `during` stay reserved as ridl clause words (driftsys/ridl#370) | typl §1.4, ADR-0012 decision 4 |
```

- [ ] **Step 5: Format, lint and check the links**

Run:
`prim fmt docs/specification/typl-language-reference.md docs/specification/ridl-language-reference.md docs/book/getting-started.md docs/specification/ridl-family-overview.md`,
then `just check`, `just link-check`, `just doc-path-check`, and
`cargo test -p ridl --locked --test book_examples` (the book file changed, no
fence did).

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/specification/typl-language-reference.md \
  docs/specification/ridl-language-reference.md docs/book/getting-started.md \
  docs/specification/ridl-family-overview.md
git commit -m "docs(typl): state the member-start rule in place of the family-wide reservation

typl §1.4 lists the registry as built and states that a domain spelling
is a keyword only as the first token of an interface member, in a
profile that implements it. ridl §2.3 and the getting-started chapter
point at the rule.

Refs driftsys/ridl#370.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 3: amend ADR-0012 decision 4

**Files:**

- Modify: `docs/decisions/ADR-0012-interaction-boundary-model.md` (`## Status`,
  after the 2026-08-03 "where the extensions live" amendment; and one paragraph
  appended to decision 4)

**Interfaces:**

- Consumes: typl §1.4 as Task 2 leaves it.
- Produces: the amendment that Tasks 7 and 10 add their own notes after.

- [ ] **Step 1: Add the amendment paragraph under `## Status`**

After the paragraph that begins
`**Amendment, 2026-08-03 — where the extensions
live.**` and before
`The exploration that produced it is recorded in`, insert:

```markdown
**Amendment, 2026-10-11 — the spellings are not reserved words (decision 4).**
Decision 4 names the keyword per (kind, family) combination; typl §1.4 read that
as a family-wide reservation of every spelling. The reservation is replaced by
the member-start rule: a spelling is a keyword only as the first token of an
interface member, in a profile that implements it, and an identifier everywhere
else. An interface member line never starts with an identifier, so a profile
that gains a spelling invalidates no source that compiled before, and the
`keywords` freeze dump only gains a line (ADR-0029). The ROS 2 corpus, which
maps a published interface set, names a field `present` and a query `trigger`;
a family-wide reservation would have broken it. In the same amendment the uxdl
words this record retired leave the implemented registry, except `states` and
`during`, which the general form (§4.8) names as ridl clauses; the rmdl words
stay reserved. Recorded under decision 4. Issue driftsys/ridl#370.
```

- [ ] **Step 2: Append the note to decision 4**

After the last paragraph of decision 4 (the one that begins
`The empty cells are
consequences, not gaps`), add, at the same indentation as
the decision's paragraphs:

```markdown
_Amended 2026-10-11:_ none of these spellings is a reserved word. A spelling
is a keyword only as the first token of an interface member, in a profile
that implements it (typl §1.4, the member-start rule). The retired uxdl words
are identifiers; `states` and `during` stay reserved as ridl clause words.
```

- [ ] **Step 3: Update the README summary if it states the reservation**

Run: `grep -n "reserved" docs/decisions/README.md`. If the ADR-0012 summary line
says the spellings are reserved, append "; amended 2026-10-11: the spellings are
not reserved words (the member-start rule)" to that line. If it does not, change
nothing.

- [ ] **Step 4: Format, lint, check**

Run:
`prim fmt docs/decisions/ADR-0012-interaction-boundary-model.md docs/decisions/README.md`,
`just check`, `just link-check`.

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add docs/decisions/ADR-0012-interaction-boundary-model.md docs/decisions/README.md
git commit -m "docs(adr): amend ADR-0012 decision 4 with the member-start rule

Refs driftsys/ridl#370.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 6: Run the full gate and open the pull request**

Run: `just build`. Expected: PASS. Rebase onto `main`, run
`cargo xtask freeze update`, confirm `git status --short freeze/` is empty or
shows only `freeze/keywords.txt`, then open the pull request from
`feat/reserved-words-370` with `Closes driftsys/ridl#370` in its body.

---

## PR 2 — the attribute registry (driftsys/ridl#268, driftsys/ridl#269)

Branch `feat/attribute-registry`, cut from `origin/main` after PR 1 merges. Spec
section 4. Not breaking: every key that compiles today still compiles;
`ridl diff` reports more. Before the pull request merges, rebase onto `main` and
run `cargo xtask freeze update`; `freeze/keywords.txt` gains the `attribute`
lines of Task 7 and `freeze/codegen-protocol.txt` gains the lines of the fields
Task 6 adds to `system.proto`. No line is removed.

### Task 4: the registry module in `ridl-core`

**Files:**

- Create: `crates/ridl-core/src/attributes.rs`
- Modify: `crates/ridl-core/src/lib.rs` (add `pub mod attributes;` in
  alphabetical position, between `cache` and `catalog_history`)

**Interfaces:**

- Consumes: nothing.
- Produces, all `pub` in `ridl_core::attributes`:
  - `enum AttrName { Key(&'static str), Namespaced }`
  - `enum Owner { Core, Backend }`
  - `enum Form { Flag, Assignment, Predicate, FlagOrAssignment }` with
    `fn word(self) -> &'static str` (`flag`, `assignment`, `predicate`,
    `flag-or-assignment`)
  - `enum Site { Command, Query, System, Component, Distribution, Deployment, Machine, Placement, Line }`
    with `fn word(self) -> &'static str` (`command`, `query`, `system`,
    `component`, `distribution`, `deployment`, `machine`, `placement`, `line`)
  - `enum NotReported { Sizing }`
  - `enum Diff { Compared(&'static str), NotReported(NotReported) }`
  - `struct AttrKey { name: AttrName, owner: Owner, form: Form, sites: &'static [Site], consumer: &'static str, diff: Diff }`
    with `fn key(&self) -> &'static str` (the key, or `ns.key` for the pattern
    row) and `fn allows(&self, site: Site) -> bool`
  - `const ATTRIBUTES: &[AttrKey]`
  - `fn lookup(key: &str) -> Option<&'static AttrKey>` — the `Key` row named
    `key`, never the pattern row
  - `fn backend_pattern() -> &'static AttrKey` — the `Namespaced` row

- [ ] **Step 1: Create the module with an empty table and the failing tests**

Write `crates/ridl-core/src/attributes.rs`:

```rust
//! The attribute registry (ADR-0012 decision 8, as amended on 2026-10-11): one
//! row per attribute key any profile accepts. The ridl checker and the rsdl
//! attribute reader look a key up here and nowhere else; `ridl diff` reports a
//! change to every `Compared` row and records the reason for every row it does
//! not report.
//!
//! A row names its machine consumer, because an attribute nobody consumes is
//! documentation wearing brackets (general form §4.1). The `keywords` freeze
//! dump holds one line per (key, site, form) of this table, so removing a key
//! or a site fails `cargo xtask freeze check`.

/// The key of a row: a plain word, or the `namespace.key` pattern of the
/// backend keys (rsdl reference §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrName {
    Key(&'static str),
    Namespaced,
}

/// Who defines the key: the family, or a backend that claims the namespace
/// through `[backend.<name>]` in the manifest. A `user:` owner waits for a
/// consumer (ADR-0012 decision 8, as amended on 2026-10-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Core,
    Backend,
}

/// The syntactic form of the key (general form §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Flag,
    Assignment,
    Predicate,
    /// A backend key is a flag or an assignment, as the backend decides.
    FlagOrAssignment,
}

impl Form {
    /// The word the `keywords` dump prints.
    pub fn word(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Assignment => "assignment",
            Self::Predicate => "predicate",
            Self::FlagOrAssignment => "flag-or-assignment",
        }
    }
}

/// A declaration kind or body line that takes an attribute block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    Command,
    Query,
    System,
    Component,
    Distribution,
    Deployment,
    Machine,
    /// A member line of a `machine` (rsdl reference §5).
    Placement,
    /// Any other body line: a system or distribution member line, an
    /// `offers` or a `requires` line.
    Line,
}

impl Site {
    /// The word the `keywords` dump prints.
    pub fn word(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Query => "query",
            Self::System => "system",
            Self::Component => "component",
            Self::Distribution => "distribution",
            Self::Deployment => "deployment",
            Self::Machine => "machine",
            Self::Placement => "placement",
            Self::Line => "line",
        }
    }
}

/// Why `ridl diff` does not report a change to a key. The set is closed: a
/// new reason is a design decision, not an edit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotReported {
    /// A sizing value changes no link and no contract (rsdl reference §14).
    Sizing,
}

/// What `ridl diff` does with a change to the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diff {
    /// Reported; the text names the category or the heading it is listed
    /// under.
    Compared(&'static str),
    NotReported(NotReported),
}

/// One row of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrKey {
    pub name: AttrName,
    pub owner: Owner,
    pub form: Form,
    /// The sites that accept the key. A key on any other site is FORM-107.
    pub sites: &'static [Site],
    /// The machine consumer (general form §4.1).
    pub consumer: &'static str,
    pub diff: Diff,
}

impl AttrKey {
    /// The key as written, `ns.key` for the backend pattern.
    pub fn key(&self) -> &'static str {
        match self.name {
            AttrName::Key(key) => key,
            AttrName::Namespaced => "ns.key",
        }
    }

    /// Whether `site` accepts the key.
    pub fn allows(&self, site: Site) -> bool {
        self.sites.contains(&site)
    }
}

const RSDL_DECLARATIONS: &[Site] = &[
    Site::System,
    Site::Component,
    Site::Distribution,
    Site::Deployment,
    Site::Machine,
];

const SIZING_SITES: &[Site] = &[Site::Deployment, Site::Placement];

/// The registry. Every key any profile accepts is a row here.
pub const ATTRIBUTES: &[AttrKey] = &[];

/// The row of a plain key, or `None` for a key no row names. The backend
/// pattern is never returned: a dotted key is read through
/// [`backend_pattern`].
pub fn lookup(key: &str) -> Option<&'static AttrKey> {
    ATTRIBUTES
        .iter()
        .find(|row| matches!(row.name, AttrName::Key(name) if name == key))
}

/// The `namespace.key` row.
pub fn backend_pattern() -> &'static AttrKey {
    ATTRIBUTES
        .iter()
        .find(|row| row.name == AttrName::Namespaced)
        .expect("the registry holds the backend pattern row")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_rows_share_a_name() {
        for (i, row) in ATTRIBUTES.iter().enumerate() {
            assert!(
                !ATTRIBUTES[..i].iter().any(|earlier| earlier.name == row.name),
                "duplicate row `{}`",
                row.key()
            );
        }
    }

    #[test]
    fn every_row_has_a_consumer_and_a_site() {
        for row in ATTRIBUTES {
            assert!(!row.consumer.is_empty(), "`{}` names no consumer", row.key());
            assert!(!row.sites.is_empty(), "`{}` has no site", row.key());
        }
    }

    #[test]
    fn the_registry_holds_every_key_the_profiles_accept() {
        // The ridl §13 predicates, the rsdl §5 keys, and the backend pattern.
        for key in [
            "require", "ensure", "instances", "external", "tier", "labels", "deprecated",
            "depth", "slots", "budget",
        ] {
            assert!(lookup(key).is_some(), "`{key}` has no row");
        }
        assert_eq!(backend_pattern().owner, Owner::Backend);
        assert_eq!(ATTRIBUTES.len(), 11, "one row per key, plus the pattern row");
    }

    #[test]
    fn the_sites_restate_the_checkers_allow_lists() {
        let allows = |key: &str, site: Site| lookup(key).unwrap().allows(site);
        assert!(allows("require", Site::Command) && allows("require", Site::Query));
        assert!(allows("ensure", Site::Query) && !allows("ensure", Site::Command));
        assert!(allows("instances", Site::Component) && !allows("instances", Site::System));
        assert!(allows("external", Site::Component) && allows("external", Site::Machine));
        assert!(allows("tier", Site::Distribution) && !allows("tier", Site::Component));
        for key in ["labels", "deprecated"] {
            for site in RSDL_DECLARATIONS {
                assert!(allows(key, *site), "`{key}` on {site:?}");
            }
            assert!(!allows(key, Site::Placement) && !allows(key, Site::Line));
            assert!(!allows(key, Site::Command), "`{key}` is not a ridl member key");
        }
        for key in ["depth", "slots", "budget"] {
            assert!(allows(key, Site::Deployment) && allows(key, Site::Placement));
            assert!(!allows(key, Site::Component));
            assert_eq!(lookup(key).unwrap().diff, Diff::NotReported(NotReported::Sizing));
        }
        for site in RSDL_DECLARATIONS.iter().chain(SIZING_SITES).chain(&[Site::Line]) {
            assert!(backend_pattern().allows(*site), "backend key on {site:?}");
        }
        assert!(!backend_pattern().allows(Site::Command));
    }

    #[test]
    fn a_dotted_key_is_not_looked_up_as_a_plain_key() {
        assert!(lookup("ns.key").is_none());
        assert!(lookup("linux.cpuset").is_none());
    }
}
```

Add `pub mod attributes;` to `crates/ridl-core/src/lib.rs` after
`pub mod cache;` (it needs no feature gate: the module has no dependency).

- [ ] **Step 2: Run the tests to see them fail on their assertions**

Run: `cargo test -p ridl-core --locked attributes`

Expected: `the_registry_holds_every_key_the_profiles_accept` FAILS with
`` `require` has no row ``; `the_sites_restate_the_checkers_allow_lists` FAILS
on `unwrap()` of a `None`. The two governance tests pass over an empty table.

- [ ] **Step 3: Fill the table**

Replace `pub const ATTRIBUTES: &[AttrKey] = &[];` with:

```rust
/// The registry. Every key any profile accepts is a row here. The sites
/// restate what the ridl checker and the rsdl attribute reader accepted
/// before this table existed; the design that added it changed none of them.
pub const ATTRIBUTES: &[AttrKey] = &[
    AttrKey {
        name: AttrName::Key("require"),
        owner: Owner::Core,
        form: Form::Predicate,
        sites: &[Site::Command, Site::Query],
        consumer: "the precondition check (ridl reference §13): generated clause code, \
                   the test plane and the online observer",
        diff: Diff::Compared("the interface category `contract_changed`"),
    },
    AttrKey {
        name: AttrName::Key("ensure"),
        owner: Owner::Core,
        form: Form::Predicate,
        sites: &[Site::Query],
        consumer: "the postcondition check (ridl reference §13): generated clause code, \
                   the test plane and the online observer",
        diff: Diff::Compared("the interface category `contract_changed`"),
    },
    AttrKey {
        name: AttrName::Key("instances"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: &[Site::Component],
        consumer: "the instance set of the component (rsdl reference §7)",
        diff: Diff::Compared("`composition changed`"),
    },
    AttrKey {
        name: AttrName::Key("external"),
        owner: Owner::Core,
        form: Form::Flag,
        sites: &[Site::Component, Site::Machine],
        consumer: "placement and the surface set (rsdl reference §9, §13)",
        diff: Diff::Compared("`composition changed` on a component, `placement changed` on a machine"),
    },
    AttrKey {
        name: AttrName::Key("tier"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: &[Site::Distribution],
        consumer: "RSDL-901, the tier inversion check (rsdl reference §3.3)",
        diff: Diff::Compared("`composition changed`"),
    },
    AttrKey {
        name: AttrName::Key("labels"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: RSDL_DECLARATIONS,
        consumer: "assurance profiles (typl reference §14.3, general form §4.7)",
        diff: Diff::Compared("`composition changed` on a system, a component or a distribution, \
                              `placement changed` on a deployment or a machine"),
    },
    AttrKey {
        name: AttrName::Key("deprecated"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: RSDL_DECLARATIONS,
        consumer: "the family lint (general form §4.7)",
        diff: Diff::Compared("`composition changed` on a system, a component or a distribution, \
                              `placement changed` on a deployment or a machine"),
    },
    AttrKey {
        name: AttrName::Key("depth"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the ring depth of an event channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Key("slots"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the slot count of a command or query channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Key("budget"),
        owner: Owner::Core,
        form: Form::Assignment,
        sites: SIZING_SITES,
        consumer: "the byte budget of a command or query channel (rsdl reference §13)",
        diff: Diff::NotReported(NotReported::Sizing),
    },
    AttrKey {
        name: AttrName::Namespaced,
        owner: Owner::Backend,
        form: Form::FlagOrAssignment,
        sites: &[
            Site::System,
            Site::Component,
            Site::Distribution,
            Site::Deployment,
            Site::Machine,
            Site::Placement,
            Site::Line,
        ],
        consumer: "the backend that claims the namespace, uninterpreted (rsdl reference §5)",
        diff: Diff::Compared("`composition changed` or `placement changed`, by the site"),
    },
];
```

- [ ] **Step 4: Run the tests, clippy and fmt**

Run: `cargo test -p ridl-core --locked attributes`,
`cargo clippy -p ridl-core --all-targets -- -D warnings`,
`cargo fmt --all --check`, and `cargo check -p ridl-core --no-default-features`
(the module must build without `fs`; `just wasm-check` runs the same for
wasm32).

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core/src/attributes.rs crates/ridl-core/src/lib.rs
git commit -m "feat(ridl-core): add the attribute registry

One table, ridl_core::attributes::ATTRIBUTES, with one row per key any
profile accepts: name, owner, form, sites, consumer and what ridl diff
does with a change. The sites restate what the checkers accept today.

Refs driftsys/ridl#268.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 5: the checkers read the registry

**Files:**

- Modify: `crates/ridl-sem/src/check.rs:5533-5607` (`check_member_attrs`),
  `:6124-6146` (`MemberKind::noun`, `GF_ATTRIBUTE_KEYS`), `:12784-12816` (the
  test `form_106_107_108_attribute_allow_list`)
- Modify: `crates/ridl-sem/src/rsdl/attrs.rs:1-16` (module doc), `:27-67`
  (`AttrSite`), `:68-78` (`RSDL_KEYS`), `:135-170` (the lookup in `read`)
- Modify: `crates/ridl-sem/src/rsdl/mod.rs:950-1012` (the test table
  `attribute_keys_follow_the_rsdl_allow_list`)

**Interfaces:**

- Consumes: `ridl_core::attributes::{lookup, backend_pattern, Site}` from
  Task 4.
- Produces: no new names. `GF_ATTRIBUTE_KEYS` and `RSDL_KEYS` are deleted;
  `AttrSite::allows` is deleted and `AttrSite` gains
  `fn registry_site(self) -> ridl_core::attributes::Site`.

- [ ] **Step 1: Write the failing ridl checker test**

In `crates/ridl-sem/src/check.rs`, in `form_106_107_108_attribute_allow_list`,
change the first case: `persist` has no consumer and no row, so it is now
FORM-106:

```rust
let known = check_ridl(
    "app",
    &format!("{PRELUDE}interface I {{\n  command c() [ persist ] @[..50ms]\n}}\n"),
);
assert_eq!(codes(&known), vec!["FORM-106"]);
assert!(
    known.diagnostics[0].message.contains("persist"),
    "got: {}",
    known.diagnostics[0].message,
);
```

and the `duplicate` case's expectation to
`vec!["FORM-106", "FORM-106", "FORM-108"]`. Replace the test's doc comment with:

```rust
// The registry (ridl_core::attributes): a key with a row that does not
// name this site is FORM-107, a key with no row FORM-106, a key twice
// FORM-108. `persist` has no row, so it moved from FORM-107 to FORM-106.
```

Then add, after that test:

```rust
#[test]
fn a_registered_rsdl_key_on_a_member_is_form_107() {
    // `tier` has a row whose sites are rsdl declarations only.
    let checked = check_ridl(
        "app",
        &format!("{PRELUDE}interface I {{\n  command c() [ tier = PLATFORM ] @[..50ms]\n}}\n"),
    );
    assert_eq!(codes(&checked), vec!["FORM-107"]);
}
```

- [ ] **Step 2: Run the ridl checker tests to see them fail on their
      assertions**

Run:
`cargo test -p ridl-sem --locked form_106_107_108_attribute_allow_list a_registered_rsdl_key_on_a_member_is_form_107`

Expected: the first FAILS with `left: ["FORM-107"], right: ["FORM-106"]`; the
second FAILS with `left: ["FORM-106"], right: ["FORM-107"]`.

- [ ] **Step 3: Make `check_member_attrs` read the registry**

In `crates/ridl-sem/src/check.rs`, add to `impl MemberKind` (next to `noun`):

```rust
/// The registry site of the member, or `None` for a kind no row names: a
/// key on a signal, an event or a fixed is then FORM-107 when it has a row
/// and FORM-106 when it has none.
fn registry_site(self) -> Option<ridl_core::attributes::Site> {
    use ridl_core::attributes::Site;
    match self {
        Self::Command => Some(Site::Command),
        Self::Query => Some(Site::Query),
        Self::Signal | Self::Event | Self::Fixed => None,
    }
}
```

Replace the `None =>` arm's lookup in `check_member_attrs`:

```rust
let range = attribute.syntax().text_range();
match ridl_core::attributes::lookup(&key) {
    Some(_) => self.error(
        DiagCode::FORM_107,
        range,
        format!("attribute `{key}` not valid on {noun}"),
    ),
    None => self.error(
        DiagCode::FORM_106,
        range,
        format!("unknown attribute key `{key}`"),
    ),
}
```

(A row that allows a member site does not exist for a plain key — the two member
rows are the predicates, which parse as predicates — so a found row is always
FORM-107 here; the predicate arms below keep the table honest.)

Make the two predicate arms read the table, keeping their codes and messages:

```rust
Some(ast::PredicateKind::Require) => {
    let allowed = kind
        .registry_site()
        .is_some_and(|site| ridl_core::attributes::lookup("require").is_some_and(|row| row.allows(site)));
    if !allowed {
        self.error(
            DiagCode::RIDL_301,
            attribute.syntax().text_range(),
            format!(
                "`require` not valid on {noun} — contracts belong to `command` and `query`"
            ),
        );
    }
}
Some(ast::PredicateKind::Ensure) => {
    let allowed = kind
        .registry_site()
        .is_some_and(|site| ridl_core::attributes::lookup("ensure").is_some_and(|row| row.allows(site)));
    if allowed {
    } else if matches!(kind, MemberKind::Command) {
        self.error(
            DiagCode::RIDL_302,
            attribute.syntax().text_range(),
            "`ensure` not valid on command — a command has no result to observe"
                .to_string(),
        );
    } else {
        self.error(
            DiagCode::RIDL_301,
            attribute.syntax().text_range(),
            format!("`ensure` not valid on {noun} — contracts belong to `query`"),
        );
    }
}
```

Delete `GF_ATTRIBUTE_KEYS` and its doc comment. Replace the doc comment of
`check_member_attrs` with:

```rust
/// The attribute registry (`ridl_core::attributes`) over the member's attr
/// block (if any): a `require`/`ensure` predicate on a site its row does
/// not name draws RIDL-301 or RIDL-302; a plain key with a row draws
/// FORM-107, because no row names a member site; a key with no row draws
/// FORM-106; a repeated key draws FORM-108.
```

- [ ] **Step 4: Run the ridl checker tests**

Run: `cargo test -p ridl-sem --locked check::tests`

Expected: PASS, including the RIDL-301/302 tests that already exist.

- [ ] **Step 5: Write the failing rsdl test rows (Review Focus 2)**

In `crates/ridl-sem/src/rsdl/mod.rs`, in the `cases` table of
`attribute_keys_follow_the_rsdl_allow_list`, after the three FORM-106 rows add:

```rust
// A namespace spelled like a core key is still a backend key.
("system S [ labels.x = 1, tier.y ] {}", &[]),
// A ridl predicate key is a reserved word in this profile and
// never reaches the attribute reader: FORM-105 at parse.
("component C [ require ] {}", &["FORM-105"]),
```

The second row's FORM-105 passes already; it pins that the registry's `require`
row can never be matched on an rsdl site. If `check` reports a second code for
the parse error, keep only the first and record the observed list.

- [ ] **Step 6: Run the rsdl tests to see the first new row's state**

Run: `cargo test -p ridl-sem --locked attribute_keys_follow_the_rsdl_allow_list`

Expected: PASS (today's reader already takes the dotted path first). The row is
a pin for the rewrite below; the semantic red of this task is Step 2.

- [ ] **Step 7: Make the rsdl reader read the registry**

In `crates/ridl-sem/src/rsdl/attrs.rs`:

Replace the `allows` method of `AttrSite` with:

```rust
/// The registry site this block sits on.
fn registry_site(self) -> ridl_core::attributes::Site {
    use ridl_core::attributes::Site;
    match self {
        Self::System => Site::System,
        Self::Component => Site::Component,
        Self::Distribution => Site::Distribution,
        Self::Deployment => Site::Deployment,
        Self::Machine => Site::Machine,
        Self::Placement => Site::Placement,
        Self::Line => Site::Line,
    }
}
```

Delete `RSDL_KEYS` and its doc comment. In `read`, replace

```rust
if !RSDL_KEYS.contains(&key.as_str()) {
    reporter.error(
        DiagCode::FORM_106,
        site,
        format!("unknown attribute key `{key}`"),
    );
    continue;
}
if !at.allows(&key) {
```

with

```rust
let Some(row) = ridl_core::attributes::lookup(&key) else {
    reporter.error(
        DiagCode::FORM_106,
        site,
        format!("unknown attribute key `{key}`"),
    );
    continue;
};
if !row.allows(at.registry_site()) {
```

In the dotted-key branch, keep the camelCase test as it is, and add one line
before `out.attrs.backend_keys.push(...)`:

```rust
debug_assert!(ridl_core::attributes::backend_pattern().allows(at.registry_site()));
```

(every rsdl site takes a backend key; the assertion documents that the pattern
row's sites are the reader's sites.)

Replace the module doc's first paragraph with:

```rust
//! The attribute blocks of the rsdl declarations and lines (rsdl reference §5).
//!
//! Every key is looked up in the attribute registry
//! (`ridl_core::attributes`), the one allow-list. A key with no row is
//! FORM-106, a key whose row does not name this site is FORM-107, and a key
//! twice in one block is FORM-108. A `namespace.key` is a backend key, carried
//! uninterpreted. The value rules of the core keys are RSDL-305 (`instances`),
//! RSDL-313 (`external`), RSDL-908 (`tier`) and RSDL-709 (`depth`, `slots`,
//! `budget`).
```

- [ ] **Step 8: Run the whole crate, clippy, fmt, and the diagnostics check**

Run: `cargo test -p ridl-sem --locked`,
`cargo clippy -p ridl-sem --all-targets -- -D warnings`,
`cargo fmt --all --check`, `git diff --exit-code freeze/diagnostics.txt`.

Expected: PASS; the diagnostics dump is unchanged (no code was added or
removed).

- [ ] **Step 9: Commit**

```bash
git add crates/ridl-sem/src/check.rs crates/ridl-sem/src/rsdl/attrs.rs crates/ridl-sem/src/rsdl/mod.rs
git commit -m "feat(ridl-sem): look attribute keys up in the registry

The ridl member check and the rsdl attribute reader read
ridl_core::attributes and nothing else. GF_ATTRIBUTE_KEYS and RSDL_KEYS
are deleted. The six general-form keys with no consumer now draw
FORM-106 where they drew FORM-107; both are errors.

Refs driftsys/ridl#268.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 6: the system diff reports every `Compared` row

The spec's table says `tier` and `deprecated` become reported changes, but
`system.proto` carries neither today (its header: "`tier` and `deprecated` feed
checks … and are not lowered"). A diff can only report what reaches the IR, so
this task lowers both, as additive fields of the system artifact. The system
artifact is not an input of the catalog hash (`catalog_hash.rs` reads the
package IR only), so no hash changes.

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/system.proto` (the header comment at
  lines 30-34; `Distribution`; `System`, `Component`, `Deployment`, `Machine`)
- Modify: `crates/ridl-sem/src/rsdl/lower.rs` (the five lowering sites:
  `v2::System` at ~43, `v2::Machine` at ~257, `v2::Deployment` at ~322,
  `v2::Distribution` at ~524, `v2::Component` — find it with
  `grep -n "v2::Component {" crates/ridl-sem/src/rsdl/lower.rs`)
- Modify: `crates/ridl-diff/Cargo.toml` (dev-dependency on `ridl-core` with
  `default-features = false`)
- Modify: `crates/ridl-diff/src/system.rs` (module doc, `diff_systems`,
  `compare_components`, `compare_deployments`, new helpers, tests)
- Modify: `docs/specification/rsdl-language-reference.md:769-796` (§14),
  `docs/book/rsdl.md:268-290` ("Comparing two systems")
- Regenerate: `freeze/codegen-protocol.txt`

**Interfaces:**

- Consumes: `ridl_core::attributes::{ATTRIBUTES, AttrName, Diff, Site}` (tests
  only);
  `ridl_ir::v2::{System, Component, Distribution, Deployment, Machine, Attribute, AttributeValue}`.
- Produces: `v2::Tier` enum (`TIER_UNSPECIFIED = 0`, `TIER_PLATFORM = 1`,
  `TIER_APPLICATION = 2`), `Distribution.tier: i32`, and
  `deprecated: Option<String>` on `System`, `Component`, `Distribution`,
  `Deployment` and `Machine`. `diff_systems` keeps its signature.

- [ ] **Step 1: Add the fields to `system.proto`**

In `Distribution`, after `repeated string since = 10;`:

```proto
// `tier = PLATFORM` or `tier = APPLICATION` (rsdl §3.3); unspecified when
// the declaration has no `tier` or its value drew RSDL-908.
Tier tier = 11;
// `deprecated = "reason"`: the text between the quotes (rsdl §5).
optional string deprecated = 12;
```

Add the enum next to `Attribute`:

```proto
// The tier of a distribution (rsdl §3.3).
enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_PLATFORM = 1;
  TIER_APPLICATION = 2;
}
```

Add `optional string deprecated = N;` with the same comment to `System`,
`Component`, `Deployment` and `Machine`, where `N` is the next number after the
highest field of that message (read each message; for `Machine` it is 9, for
`Deployment` 16, for `Component` and `System` read the message). Replace the
header lines 30-34 with:

```proto
// repeated on the producer. The rsdl-owned keys are consumed into the facts
// they feed (rsdl §5): `instances` and `external` are fields, `depth`,
// `slots` and `budget` are the `Sizing` of a deployment and of a placement,
// `tier` is a field of the distribution and `deprecated` of every
// declaration, so that `ridl diff` reports a change to each (rsdl §14).
```

- [ ] **Step 2: Lower `tier` and `deprecated`**

In `crates/ridl-sem/src/rsdl/lower.rs`, at each of the five literals add
`deprecated: decl.attrs.deprecated.clone(),` (for `Machine`:
`machine.attrs.deprecated.clone()`), and in the `v2::Distribution` literal add:

```rust
tier: match decl.tier {
    Some(Tier::Platform) => v2::Tier::Platform as i32,
    Some(Tier::Application) => v2::Tier::Application as i32,
    None => v2::Tier::Unspecified as i32,
},
```

importing `super::Tier` if the module does not already. Add to the lowering
tests of that file, next to
`distribution_lines_keep_their_order_and_backend_keys`, a test that lowers
`distribution D [ tier = PLATFORM, deprecated = "x" ] { C }` over the file's
existing fixture and asserts
`system.distributions[0].tier == v2::Tier::Platform as i32` and
`system.distributions[0].deprecated.as_deref() == Some("x")`. Run it first to
see it fail on the assertion (`tier` is 0 until the literal is edited), then
make it pass.

Run: `cargo test -p ridl-sem --locked rsdl::lower`. Expected: PASS.

- [ ] **Step 3: Add the dev-dependency and write the failing diff tests**

In `crates/ridl-diff/Cargo.toml` add:

```toml
[dev-dependencies]
ridl-core = { path = "../ridl-core", version = "0.9.0", default-features = false }
```

(use the version `crates/ridl-sem/Cargo.toml` names for `ridl-core`.)

In the `tests` module of `crates/ridl-diff/src/system.rs`, add the helpers and
tests:

```rust
    fn backend(namespace: &str, key: &str, value: Option<&str>) -> v2::Attribute {
        v2::Attribute {
            namespace: namespace.to_string(),
            key: key.to_string(),
            value: value.map(|text| v2::AttributeValue {
                kind: Some(v2::attribute_value::Kind::Scalar(text.to_string())),
            }),
        }
    }

    /// `base()` plus one distribution, so that every rsdl site is present.
    fn with_distribution() -> System {
        let mut system = base();
        system.distributions.push(v2::Distribution {
            name: "Adas".to_string(),
            package: "veh.topology".to_string(),
            members: vec![v2::MemberLine {
                component: "veh.topology.Cruise".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        system
    }

    /// Every `Compared` row of the registry with an rsdl site is reported by
    /// `diff_systems`, and every `NotReported` row is not. The mutation per
    /// row is written here, so a row added to the registry without diff
    /// support fails this test on the `panic!` arm.
    #[test]
    fn the_registry_rows_are_reported_as_recorded() {
        use ridl_core::attributes::{ATTRIBUTES, AttrName, Diff, Site};
        for row in ATTRIBUTES {
            if !row.sites.iter().any(|site| !matches!(site, Site::Command | Site::Query)) {
                continue;
            }
            let old = with_distribution();
            let mut new = with_distribution();
            let key = match row.name {
                AttrName::Key(key) => key,
                AttrName::Namespaced => "ns.key",
            };
            match key {
                "instances" => new.components[0].instances = vec!["Unit".to_string()],
                "external" => new.components[0].external = true,
                "tier" => new.distributions[0].tier = v2::Tier::Application as i32,
                "labels" => new.distributions[0].labels = vec!["QM".to_string()],
                "deprecated" => new.deployments[0].deprecated = Some("use v2".to_string()),
                "depth" => new.deployments[0].sizing = Some(v2::Sizing { depth: Some(4), slots: None, budget: None }),
                "slots" => new.deployments[0].placements[0].sizing = Some(v2::Sizing { depth: None, slots: Some(4), budget: None }),
                "budget" => new.deployments[0].sizing = Some(v2::Sizing { depth: None, slots: None, budget: Some(4096) }),
                "ns.key" => new.deployments[0].machines[0].attributes.push(backend("linux", "cpuset", Some("(2)"))),
                other => panic!("registry row `{other}` has no mutation in this test"),
            }
            let changes = diff_systems(&old, &new);
            match row.diff {
                Diff::Compared(_) => assert!(!changes.is_empty(), "`{key}` must be reported"),
                Diff::NotReported(reason) => assert!(
                    changes.is_empty(),
                    "`{key}` is recorded as not reported ({reason:?}) but was: {changes:?}"
                ),
            }
        }
    }

    /// rsdl §14: `labels`, `deprecated`, `tier` and a backend key are listed
    /// under the heading of the declaration they are on, as a before-and-after
    /// line, on every site that takes them.
    #[test]
    fn attribute_changes_are_listed_under_the_heading_of_their_site() {
        let old = with_distribution();
        let mut new = with_distribution();
        new.labels = vec!["QM".to_string()];
        new.components[0].deprecated = Some("use Cruise2".to_string());
        new.distributions[0].tier = v2::Tier::Platform as i32;
        new.distributions[0].members[0].attributes.push(backend("sign", "key", None));
        new.components[0].offers[0].attributes.push(backend("someip", "serviceId", Some("4660")));
        new.deployments[0].attributes.push(backend("rust", "crate", Some("\"s\"")));
        new.deployments[0].machines[0].labels = vec!["ASIL_B".to_string()];
        new.deployments[0].placements[0].attributes.push(backend("linux", "cpuset", Some("(2)")));
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "composition changed: veh.topology.Vehicle/labels: () -> (QM)",
                "composition changed: veh.topology.Cruise/deprecated: (absent) -> \"use Cruise2\"",
                "composition changed: veh.topology.Cruise/offers/veh.adas.cruise/attributes: \
                 [] -> [ someip.serviceId = 4660 ]",
                "composition changed: veh.topology.Adas/tier: (absent) -> PLATFORM",
                "composition changed: veh.topology.Adas/veh.topology.Cruise/attributes: \
                 [] -> [ sign.key ]",
                "placement changed: Production/attributes: [] -> [ rust.crate = \"s\" ]",
                "placement changed: Production/AdasHpc/labels: () -> (ASIL_B)",
                "placement changed: Production/veh.topology.Cruise.primary/attributes: \
                 [] -> [ linux.cpuset = (2) ]",
            ]
        );
    }

    /// Review Focus 3: a value removed is reported, as the mirror of one added.
    #[test]
    fn an_attribute_removed_is_reported() {
        let mut old = with_distribution();
        old.components[0].deprecated = Some("use Cruise2".to_string());
        old.distributions[0].tier = v2::Tier::Platform as i32;
        let new = with_distribution();
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "composition changed: veh.topology.Cruise/deprecated: \"use Cruise2\" -> (removed)",
                "composition changed: veh.topology.Adas/tier: PLATFORM -> (removed)",
            ]
        );
    }
```

Read `crate::push_values` to confirm how it prints an absent `before`
(`(absent)`) and an absent `after` (`(removed)`); the expected strings above
assume the texts the existing tests show. The list value of `labels` is rendered
`(A, B)` and an empty one `()`; an attribute list is rendered
`[ ns.key = value, ns.flag ]` and an empty one `[]`; a `Scalar` value is printed
as its text and a `List` as `(a, b)`.

- [ ] **Step 4: Run the tests to see them fail on their assertions**

Run: `cargo test -p ridl-diff --locked system::tests`

Expected: `the_registry_rows_are_reported_as_recorded` FAILS with
`` `tier` must be reported ``; the other two FAIL on their `assert_eq!`.
`system_sizing_changes_are_not_reported` still passes.

- [ ] **Step 5: Compare the attributes in `diff_systems`**

In `crates/ridl-diff/src/system.rs` add the helpers:

```rust
/// `(A, B)` as `labels` is written; `()` when empty.
fn label_list(labels: &[String]) -> String {
    format!("({})", labels.join(", "))
}

/// `[ ns.key = value, ns.flag ]` as a backend key block is written; `[]`
/// when empty.
fn attribute_list(attributes: &[v2::Attribute]) -> String {
    if attributes.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = attributes
        .iter()
        .map(|attribute| match &attribute.value {
            Some(value) => format!(
                "{}.{} = {}",
                attribute.namespace,
                attribute.key,
                value_text(value)
            ),
            None => format!("{}.{}", attribute.namespace, attribute.key),
        })
        .collect();
    format!("[ {} ]", items.join(", "))
}

fn value_text(value: &v2::AttributeValue) -> String {
    match &value.kind {
        Some(v2::attribute_value::Kind::Scalar(text)) => text.clone(),
        Some(v2::attribute_value::Kind::List(list)) => {
            let items: Vec<String> = list.items.iter().map(value_text).collect();
            format!("({})", items.join(", "))
        }
        None => String::new(),
    }
}

fn tier_text(tier: i32) -> Option<String> {
    match v2::Tier::try_from(tier) {
        Ok(v2::Tier::Platform) => Some("PLATFORM".to_string()),
        Ok(v2::Tier::Application) => Some("APPLICATION".to_string()),
        _ => None,
    }
}

/// One before-and-after line when the two values differ.
fn value_change(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: String,
    before: Option<String>,
    after: Option<String>,
) {
    if before != after {
        changes.push(SystemChange {
            heading,
            path,
            before,
            after,
        });
    }
}

/// The `labels`, `deprecated` and backend keys of a declaration, under
/// `heading` at `path`.
fn declaration_attrs(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: &str,
    old: (&[String], Option<&String>, &[v2::Attribute]),
    new: (&[String], Option<&String>, &[v2::Attribute]),
) {
    value_change(
        changes,
        heading,
        format!("{path}/labels"),
        Some(label_list(old.0)),
        Some(label_list(new.0)),
    );
    value_change(
        changes,
        heading,
        format!("{path}/deprecated"),
        old.1.map(|reason| format!("{reason:?}")),
        new.1.map(|reason| format!("{reason:?}")),
    );
    line_attrs(changes, heading, path, old.2, new.2);
}

/// The backend keys of a line, under `heading` at `path`.
fn line_attrs(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: &str,
    old: &[v2::Attribute],
    new: &[v2::Attribute],
) {
    value_change(
        changes,
        heading,
        format!("{path}/attributes"),
        Some(attribute_list(old)),
        Some(attribute_list(new)),
    );
}
```

Then wire them in, keeping the existing order of the report (composition first,
then placement):

- At the start of `diff_systems`, before the component loop, compare the
  system's own declaration:
  `declaration_attrs(&mut changes, SystemHeading::CompositionChanged, &format!("{}.{}", new.package, new.name), (&old.labels, old.deprecated.as_ref(), &old.attributes), (&new.labels, new.deprecated.as_ref(), &new.attributes));`
  and its member lines, paired by `component`: for each `line` of `new.members`
  with a match in `old.members` on `component`,
  `line_attrs(&mut changes, SystemHeading::CompositionChanged, &format!("{}.{}/{}", new.package, new.name, line.component), &before.attributes, &line.attributes)`.
- In `compare_components`, after the `external` comparison: the declaration
  (`declaration_attrs` at `name`), then each `offers` line paired by `service`
  (`line_attrs` at `{name}/offers/{service}`) and each `requires` line paired by
  `interface_text` (`line_attrs` at `{name}/requires/{interface}`).
- After the component loops and before the deployment loops, compare the
  distributions paired by `qualified_name()`: `tier`
  (`value_change(..., CompositionChanged, format!("{name}/tier"), tier_text(before.tier), tier_text(after.tier))`),
  then `declaration_attrs` at `name`, then each member line paired by
  `component` (`line_attrs` at `{name}/{component}`). A distribution present on
  one side only is not compared: the distribution set is not a composition
  change today, and this task does not widen that.
- In `compare_deployments`, before the machine loop: `declaration_attrs` at
  `new.name` under `PlacementChanged`; in the `Some(before)` arm of the machine
  loop, after the `external` comparison, `declaration_attrs` at
  `{deployment}/{machine}` under `PlacementChanged`; in the placement loop, pair
  on `component` and `instance` (not only moved ones) and call `line_attrs` at
  `{deployment}/{component}.{instance}` under `PlacementChanged`.

Update the module doc: in the "Placement changed" bullet add "a `labels`,
`deprecated` or backend key of the deployment, of a machine or of a placement
line changed"; in the "Composition changed" bullet add "a `labels`,
`deprecated`, `tier` or backend key of the system, of a component, of a
distribution or of one of their lines changed". Replace the sentence of the
`diff_systems` doc that begins `The sizing fields` with: "The sizing fields
(`Deployment.sizing`, `Placement.sizing`) are never read: the registry records
them as not reported, reason sizing (`ridl_core::attributes`)."

- [ ] **Step 6: Run the tests, adjust the expected strings to the rendered form,
      and run the crate**

Run: `cargo test -p ridl-diff --locked`

Expected: PASS. If the rendered `(absent)`/`(removed)` words differ from the
expected strings in Step 3, change the test strings to what `push_values` prints
— the assertion is on the path and the values, not on those two words.
`system_changes_render_under_their_headings_with_no_verdict` and the two heading
tests must still pass unchanged: the base system has no attribute, so no new
line appears in them.

- [ ] **Step 7: Regenerate the protocol dump and check the diagnostics**

Run: `cargo xtask freeze update`, `git status --short freeze/`,
`cargo xtask freeze check`, `git diff --exit-code freeze/diagnostics.txt`.

Expected: only `freeze/codegen-protocol.txt` changed, with added lines
(`ridl.ir.v2.Distribution.tier = 11 singular ridl.ir.v2.Tier`,
`ridl.ir.v2.Tier values TIER_APPLICATION TIER_PLATFORM TIER_UNSPECIFIED`, the
three `Tier` value lines, and one `deprecated` line per message); no line
removed; the check passes.

- [ ] **Step 8: Update rsdl §14 and the book chapter**

In `docs/specification/rsdl-language-reference.md` §14, replace the two bullets'
sentences about what is listed:

- In the "placement changed" bullet, replace
  `A changed`depth`,`slots`or`budget`, on the deployment or on a placement line, is not listed: ...`
  with:

```markdown
A changed `labels`, `deprecated` or backend key on the deployment, on a
machine or on a placement line is listed under the same heading, as a
before-and-after line. A changed `depth`, `slots` or `budget`, on the
deployment or on a placement line, is not listed: the attribute registry
records the three sizing keys as not reported, with the reason that a sizing
value changes no link and no contract, and `ridl diff` reads neither
`Deployment.sizing` nor `Placement.sizing`.
```

- In the "composition changed" bullet, after `a component made`external``,
  insert
  `, a`labels`,`deprecated`,`tier`or backend key changed on the
  system, on a component, on a distribution or on one of their lines`.

In `docs/book/rsdl.md`, "Comparing two systems", after the sentence that ends
`are listed under`composition changed`instead.` add:

```markdown
A changed `labels`, `deprecated`, `tier` or backend key is listed under the
heading of the declaration it is on, as a before-and-after line; a changed
sizing key (`depth`, `slots`, `budget`) is not listed, because it changes no
link and no contract.
```

Run: `prim fmt docs/specification/rsdl-language-reference.md docs/book/rsdl.md`,
`just check`, `cargo test -p ridl --locked --test book_examples`.

- [ ] **Step 9: Clippy, fmt, commit**

Run:
`cargo clippy -p ridl-diff -p ridl-sem -p ridl-ir --all-targets -- -D warnings`,
`cargo fmt --all --check`.

```bash
git add crates/ridl-ir/proto/ridl/ir/v2/system.proto crates/ridl-sem/src/rsdl/lower.rs \
  crates/ridl-diff/Cargo.toml crates/ridl-diff/src/system.rs Cargo.lock \
  freeze/codegen-protocol.txt docs/specification/rsdl-language-reference.md docs/book/rsdl.md
git commit -m "feat(ridl-diff): report every compared attribute of the system

tier and deprecated are lowered into the system artifact, and the system
diff lists a change to labels, deprecated, tier or a backend key under
the heading of its site. A test iterates the registry: every Compared
row is reported and every NotReported row is not.

Refs driftsys/ridl#269.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 7: the registry in the freeze dump, the references and ADR-0012

**Files:**

- Modify: `xtask/src/freeze/keywords.rs` (`dump`, tests)
- Regenerate: `freeze/keywords.txt`
- Modify: `docs/specification/ridl-language-reference.md:1356-1396` (§13, the
  attribute table)
- Modify: `docs/specification/rsdl-language-reference.md:385-455` (§5, the table
  and the first paragraph)
- Modify: `docs/decisions/ADR-0012-interaction-boundary-model.md` (`## Status`
  and decisions 8 and 9)
- Modify: `docs/specification/ridl-family-overview.md` (§5, one row)

**Interfaces:**

- Consumes: `ridl_core::attributes::{ATTRIBUTES, AttrKey, Form, Site}`.
- Produces: `attribute <key> <site> <form>` lines in `freeze/keywords.txt`.

- [ ] **Step 1: Write the failing dump test**

In `xtask/src/freeze/keywords.rs` tests, add:

```rust
#[test]
fn keywords_dump_lists_the_attribute_registry() {
    let lines = dump(Path::new(".")).unwrap();
    for expected in [
        "attribute tier distribution assignment",
        "attribute require command predicate",
        "attribute depth placement assignment",
        "attribute ns.key line flag-or-assignment",
    ] {
        assert!(lines.iter().any(|l| l == expected), "{expected}: {lines:?}");
    }
    // One line per (key, site, form): widening an allow-list adds lines,
    // removing a key or a site removes one.
    let attribute_lines = lines.iter().filter(|l| l.starts_with("attribute ")).count();
    let expected: usize = ridl_core::attributes::ATTRIBUTES
        .iter()
        .map(|row| row.sites.len())
        .sum();
    assert_eq!(attribute_lines, expected);
}
```

- [ ] **Step 2: Run it to see it fail on its assertion**

Run: `cargo test -p xtask --locked keywords_dump_lists_the_attribute_registry`

Expected: FAIL with `attribute tier distribution assignment: [...]`.

- [ ] **Step 3: Add the lines to the dump**

In `xtask/src/freeze/keywords.rs`, add:

```rust
/// `attribute <key> <site> <form>` for each site of each row of the attribute
/// registry. A key or a site removed from the registry removes a line.
fn attribute_lines() -> Vec<String> {
    ridl_core::attributes::ATTRIBUTES
        .iter()
        .flat_map(|row| {
            row.sites
                .iter()
                .map(move |site| format!("attribute {} {} {}", row.key(), site.word(), row.form.word()))
        })
        .collect()
}
```

and make `dump` return `lines(...)` followed by `attribute_lines()`:

```rust
pub fn dump(_root: &Path) -> Result<Vec<String>, String> {
    let mut lines = lines(FAMILY_RESERVED, |profile, word| {
        keyword_in(profile, word).is_some()
    });
    lines.extend(attribute_lines());
    Ok(lines)
}
```

Update the module doc to
`//! The keyword registry: which word is an active
keyword in which profile, and the attribute registry: which key is accepted on
which site.`

- [ ] **Step 4: Run the tests, regenerate, check**

Run: `cargo test -p xtask --locked keywords`, `cargo xtask freeze update`,
`git status --short freeze/` (expected: only `freeze/keywords.txt`, with 32
added `attribute` lines — 2 + 1 + 1 + 2 + 1 + 5 + 5 + 2 + 2 + 2 + 7 — and no
line removed), `cargo xtask freeze check`.

- [ ] **Step 5: List the registry as built in ridl §13**

In `docs/specification/ridl-language-reference.md` §13, after the two-row table,
add:

```markdown
Both predicates are rows of the attribute registry (`ridl_core::attributes`,
ADR-0012 decision 8): owner `core`, form predicate, sites as the table above
says, consumer the clause check, and a change to either reported by `ridl diff`
as `contract_changed`. The registry is the one allow-list the checker reads: a
plain key on a member draws FORM-107 when the registry has a row for it on an
rsdl site and FORM-106 when it has none. No plain key is accepted on a member
today.
```

- [ ] **Step 6: List the registry as built in rsdl §5**

In `docs/specification/rsdl-language-reference.md` §5, replace the first
paragraph's last sentence (`The principle (D-6): ...`) with:

```markdown
The principle (D-6): **a core key is allow-listed per declaration kind and has
a machine consumer; every other key belongs to a backend and is carried
uninterpreted.** The table is the attribute registry as built
(`ridl_core::attributes`, ADR-0012 decision 8): one row per key, with its owner
(`core`, or `backend` for the `backend.key` pattern), its form, its sites, its
consumer and what `ridl diff` does with a change to it.
```

Add a `Diff` column to the table, with these cells: `instances`, `external`,
`tier`, `labels`, `deprecated` and `backend.key` — "reported under the heading
of its site (§14)"; `depth`, `slots`, `budget` — "not reported: sizing (§14)".

- [ ] **Step 7: Amend ADR-0012 decisions 8 and 9**

Under `## Status`, after the 2026-10-11 amendment of Task 3, insert:

```markdown
**Amendment, 2026-10-11 — the registry as built (decisions 8 and 9).** The
registry is `ridl_core::attributes::ATTRIBUTES`, one table that the ridl member
check and the rsdl attribute reader read as their only allow-list. Its owner
field has two values today, `core` and `backend` (a key whose namespace a
configured backend claims); the `user:<namespace>` owner of decision 8 waits for
a consumer, and no `extension:<name>` owner exists because no extension does.
The six general-form keys with no consumer (`default`, `init`, `persist`,
`invariant`, and `labels`/`deprecated` on an interface member) are not rows:
each draws FORM-106. For the rsdl system diff, which gives no verdict, decision
9's fail-closed rule reads "reported": `tier`, `labels`, `deprecated` and every
backend key are reported changes, and `depth`, `slots` and `budget` are
recorded as not reported, with the reason that a sizing value changes no link
and no contract. Recorded under decisions 8 and 9. Issues driftsys/ridl#268 and
driftsys/ridl#269.
```

Append to decision 8, at its indentation:

```markdown
_Amended 2026-10-11:_ built as `ridl_core::attributes`. The owner is `core`
or `backend`; `user:<namespace>` waits for a consumer.
```

Append to decision 9:

```markdown
_Amended 2026-10-11:_ for the system diff, which gives no verdict,
fail-closed means reported. The three sizing keys are the recorded
exception, reason sizing.
```

Add ledger row 40 to `docs/specification/ridl-family-overview.md` §5:

```markdown
| 40 | **The attribute registry as built**: one table in `ridl-core` is the only allow-list the checkers read; `require` and `ensure` are registered, the six consumer-less general-form keys are not; the system diff reports `tier`, `labels`, `deprecated` and backend keys and records the sizing keys as not reported (driftsys/ridl#268, driftsys/ridl#269) | ridl §13, rsdl §5, §14, ADR-0012 decisions 8 and 9 |
```

- [ ] **Step 8: Format, lint, check, commit**

Run:
`prim fmt docs/specification/ridl-language-reference.md docs/specification/rsdl-language-reference.md docs/decisions/ADR-0012-interaction-boundary-model.md docs/specification/ridl-family-overview.md`,
`just check`, `just link-check`, `just doc-path-check`, `just story-id-check`,
`cargo clippy -p xtask --all-targets -- -D warnings`, `cargo fmt --all --check`.

```bash
git add xtask/src/freeze/keywords.rs freeze/keywords.txt \
  docs/specification/ridl-language-reference.md docs/specification/rsdl-language-reference.md \
  docs/decisions/ADR-0012-interaction-boundary-model.md docs/specification/ridl-family-overview.md
git commit -m "feat(xtask): freeze the attribute registry in the keywords dump

One line per (key, site, form). The ridl and rsdl references list the
registry as built, and ADR-0012 decisions 8 and 9 record it.

Closes driftsys/ridl#268, closes driftsys/ridl#269.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 9: Run the full gate and open the pull request**

Run: `just build`. Expected: PASS. Rebase onto `main`, run
`cargo xtask freeze update`, confirm only `freeze/keywords.txt` and
`freeze/codegen-protocol.txt` differ from `main` and only by added lines, then
open the pull request from `feat/attribute-registry` with
`Closes driftsys/ridl#268` and `Closes driftsys/ridl#269` in its body.

---

## PR 3 — family and shape (driftsys/ridl#270)

Branch `feat/interaction-family`, cut from `origin/main` after PR 2 merges. Spec
section 5. Not breaking: additive fields; a plugin that does not know them keeps
working. Before the pull request merges, rebase onto `main` and run
`cargo xtask freeze update`; `freeze/codegen-protocol.txt` gains lines and
`freeze/wire.txt` is unchanged.

The codegen model's `Interaction` already has a `oneof shape` (its five per-kind
shapes), so a scalar field named `shape` cannot be added to it. This plan puts
`Shape shape` on `CommandShape`, which mirrors `ir.proto`, where `shape` is a
field of `CommandDef` only. The field name is unchanged; its message differs
from spec section 5.2's sentence "The codegen model's `Interaction` gains the
same two fields". This is the one place the plan departs from the spec's letter;
it needs Sebastien's confirmation before Task 8 starts.

### Task 8: the enums and fields, lowered as dispatch and none

**Files:**

- Modify: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` (`SignalDef`, `EventDef`,
  `CommandDef`, `QueryDef`; two new enums after `TimingMode`)
- Modify: `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` (`Interaction`,
  `CommandShape`; two new enums after `Kind`)
- Modify: `crates/ridl-sem/src/check.rs:4599`, `:4637`, `:5091`, `:5124` (the
  four lowering literals)
- Modify: `crates/ridl-ir/src/codegen/lower.rs:784-945` (`interaction`)
- Modify: every file that writes a `v2::SignalDef {`, `v2::EventDef {`,
  `v2::CommandDef {`, `v2::QueryDef {`, `v1::Interaction {` or
  `v1::CommandShape {` literal (find them with
  `grep -rn "SignalDef {\|EventDef {\|CommandDef {\|QueryDef {\|v1::Interaction {\|CommandShape {" crates xtask --include=*.rs | grep -v ridl-syntax`;
  today: `ridl-ir` (`catalog_hash.rs`, `lib.rs`, `rules.rs`,
  `codegen/deployment.rs`, `codegen/tests.rs`, `codegen/lower.rs`),
  `ridl-sem/src/check.rs`, `ridl-diff` (`classify/classify_tests.rs`,
  `tests.rs`, `unit_verdict.rs`, `walk.rs`), `ridl-descriptor/tests/lower.rs`,
  `ridl-backend-rust/src/{clauses.rs,claims.rs,tests.rs}`,
  `ridl-backend-proto/tests/stability.rs`,
  `ridl-backend-flatbuffers/tests/stability.rs`, `ridlc/tests/layout.rs`)
- Modify: `crates/ridl/tests/facade.rs` (one new test)
- Regenerate: `freeze/codegen-protocol.txt`

**Interfaces:**

- Consumes: nothing new.
- Produces: `v2::Family` (`Dispatch = 0`, `Presentation`, `Intent`,
  `Acquisition`, `Control`), `v2::Shape` (`None = 0`, `Activate`, `Toggle`,
  `Select`, `Adjust`, `Dismiss`), `SignalDef.family`, `EventDef.family`,
  `CommandDef.family`, `CommandDef.shape`, `QueryDef.family` (all `i32`);
  `codegen::v1::Family`, `codegen::v1::Shape`, `v1::Interaction.family`,
  `v1::CommandShape.shape`. prost strips the enum-name prefix, so the Rust
  variants are `v2::Family::Dispatch` and `v2::Shape::None`.

- [ ] **Step 1: Write the failing hash-pin test**

In `crates/ridl/tests/facade.rs`, next to
`catalog_hash_is_the_same_whether_a_build_emits_proto_flatbuffers_or_both`, add:

```rust
/// The catalog hash of `examples/cabin` is the one `freeze/wire.txt` records.
/// The IR gained `family` and `shape` with zero values that encode to no
/// byte, so the hash of every existing source is unchanged; this test holds
/// the example and the dump to each other.
#[test]
fn the_cabin_catalog_hash_matches_the_wire_dump() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/ridl sits two levels below the repository root");
    let wire = std::fs::read_to_string(root.join("freeze/wire.txt")).expect("freeze/wire.txt");
    let recorded = wire
        .lines()
        .find_map(|line| line.strip_prefix("catalog-hash veh.cabin "))
        .expect("the wire dump records the cabin catalog hash");
    let out = TempDir::new("cabin-catalog");
    let (code, stderr) = ridl(&[
        "build".as_ref(),
        root.join("examples/cabin").as_os_str(),
        "--emit".as_ref(),
        "catalog".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "examples/cabin must build, stderr:\n{stderr}");
    let descriptor = std::fs::read_dir(out.path())
        .expect("the out dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_file())
        .expect("--emit catalog writes one descriptor");
    let (code, described) = ridl_stdout(&["describe".as_ref(), descriptor.as_os_str()]);
    assert_eq!(code, 0, "`ridl describe` must exit 0");
    let json: serde_json::Value = serde_json::from_str(&described).expect("describe prints JSON");
    let hex: String = json["hash"]
        .as_array()
        .expect("the descriptor has a `hash` byte array")
        .iter()
        .map(|b| format!("{:02x}", b.as_u64().expect("a byte")))
        .collect();
    assert_eq!(hex, recorded);
}
```

`ridl(&[..])` in this file (`facade.rs:44`) returns `(code, stderr)`. Add
`ridl_stdout` next to it with the same body, except that it returns
`String::from_utf8_lossy(&output.stdout)`. Read
`xtask/src/freeze/wire.rs:177-220` for the exact `describe` output shape
(`name`, `hash` as a byte array).

- [ ] **Step 2: Run it to see it pass, then make it red by hand once**

Run:
`cargo test -p ridl --locked --test facade the_cabin_catalog_hash_matches_the_wire_dump`

Expected: PASS before any proto change. To prove the test can fail: edit
`examples/cabin`'s interface by renaming one interaction, run the test
(expected: FAIL on the `assert_eq!`), then revert the edit with
`git checkout examples/cabin`. The test is this task's guard: it must still pass
after Step 6.

- [ ] **Step 3: Add the enums and fields to the two schemas**

In `ir.proto`, after `TimingMode`:

```proto
// The interaction family (ADR-0012 decisions 2 and 6). The zero value is
// `FAMILY_DISPATCH`, not an `_UNSPECIFIED` value: every interaction that
// exists today is dispatch, proto3 encodes no byte for a zero enum field, so
// the catalog hash of every existing source is unchanged and an IR written
// before this field decodes as dispatch.
enum Family {
  FAMILY_DISPATCH = 0;
  FAMILY_PRESENTATION = 1;
  FAMILY_INTENT = 2;
  FAMILY_ACQUISITION = 3;
  FAMILY_CONTROL = 4;
}

// The operation shape of an intent command (ADR-0012 decisions 5 and 6).
// The zero value is `SHAPE_NONE` for the reason `Family` gives.
enum Shape {
  SHAPE_NONE = 0;
  SHAPE_ACTIVATE = 1;
  SHAPE_TOGGLE = 2;
  SHAPE_SELECT = 3;
  SHAPE_ADJUST = 4;
  SHAPE_DISMISS = 5;
}
```

Add to `SignalDef`: `Family family = 5;`; to `EventDef`: `Family family = 3;`;
to `CommandDef`: `Family family = 4;` and `Shape shape = 5;`; to `QueryDef`:
`Family family = 5;` — each with the comment
`// The family (ADR-0012 decision 6); dispatch for every spelling the toolchain reads today.`
and, on `shape`,
`// The operation shape; none unless the family is intent. The valid (kind, family, shape) combinations are checked by ridl_ir::class.`
`FixedDef` gets no field: `fixed` is family-neutral.

In `model.proto`, after `enum Kind`, the same two enums with the same comments
(the model mirrors the IR). Add to `Interaction`, after `reservation = 8`:
`Family family = 9;` with the comment
`// The family of the interaction (ADR-0012 decision 6), copied from the IR. A plugin that does not read it sees the core kind.`
Add to `CommandShape`: `Shape shape = 5;` with the comment
`// The operation shape of an intent command, copied from the IR. On the command shape, not on Interaction, because Interaction's oneof is named shape.`

- [ ] **Step 4: Compile and repair every struct literal**

Run: `cargo check --workspace --all-targets --locked`

For each error "missing field `family`" or "missing field `shape`", add the
field with its zero value: `family: v2::Family::Dispatch as i32,`,
`shape: v2::Shape::None as i32,` (`v1::Family`/`v1::Shape` in the codegen
model's literals). Where a test builder already passes `..Default::default()`,
nothing is needed. Do not replace a full literal with `..Default::default()`.

In `crates/ridl-sem/src/check.rs` the four lowering literals get the explicit
value with a comment on the first:
`// Every spelling the toolchain reads today lowers to dispatch and no shape (ADR-0012 decision 6, as amended on 2026-10-11).`

In `crates/ridl-ir/src/codegen/lower.rs`, `interaction` copies the values:
return `family` from the match as a fifth tuple element (`signal.family`,
`event.family`, `command.family`, `query.family`, and
`v2::Family::Dispatch as i32` for a fixed and for the `_` arm) and set `family`
on the `v1::Interaction` literal; in the `CommandShape` literal set
`shape: command.shape`.

- [ ] **Step 5: Add the lowering tests**

In `crates/ridl-sem/src/check.rs` tests, next to `signal_def`:

```rust
#[test]
fn every_spelling_lowers_to_dispatch_and_no_shape() {
    let checked = check_ridl(
        "app",
        &format!(
            "{PRELUDE}interface I {{\n  signal s : Speed @10ms\n  event e : Speed @[100ms..1s]\n  \
             command c() @[..50ms]\n  query q(): Speed @[..50ms]\n}}\n"
        ),
    );
    assert_eq!(codes(&checked), Vec::<&str>::new());
    assert_eq!(signal_def(&checked, "s").family, v2::Family::Dispatch as i32);
    assert_eq!(event_def(&checked, "e").family, v2::Family::Dispatch as i32);
    let command = command_def(&checked, "c");
    assert_eq!(command.family, v2::Family::Dispatch as i32);
    assert_eq!(command.shape, v2::Shape::None as i32);
    assert_eq!(query_def(&checked, "q").family, v2::Family::Dispatch as i32);
}
```

Add `command_def` next to `query_def` (at `check.rs:13295`) with the same shape.
This test is green at once; it is the pin for the sentence "every existing
keyword lowers to dispatch". The semantic red of this task is the codegen test
below.

In `crates/ridl-ir/src/codegen/tests.rs`, add a test that builds a package with
one
`CommandDef { family: v2::Family::Intent as i32, shape: v2::Shape::Toggle as i32, .. }`
through the file's existing `lower` helper and asserts the model's
`interactions[0].family == v1::Family::Intent as i32` and its
`Shape::Command(command)` has `command.shape == v1::Shape::Toggle as i32`. Write
it before editing `lower.rs`'s `interaction` (Step 4's last paragraph), run it
to see it fail on the `assert_eq!` (the model still carries 0), then make the
edit and see it pass.

- [ ] **Step 6: Run the workspace tests, the hash pin, and the freeze check**

Run: `cargo test --workspace --locked`

Expected: PASS. The canonical JSON emits every field (`emit_fields()` in
`crates/ridl-ir/build.rs`), so a test that compares a JSON rendering verbatim
now sees `"family": "FAMILY_DISPATCH"` and, on a command,
`"shape": "SHAPE_NONE"`; update such an expectation and nothing else. The
baseline fixture
`crates/ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json` is
read, not compared, and needs no change (Review Focus 4 pins that).

Run:
`cargo test -p ridl --locked --test facade the_cabin_catalog_hash_matches_the_wire_dump`
(expected: PASS), `cargo xtask freeze update`, `git status --short freeze/`
(expected: only `freeze/codegen-protocol.txt`, added lines only;
`freeze/wire.txt` unchanged), `cargo xtask freeze check`,
`git diff --exit-code freeze/diagnostics.txt freeze/wire.txt`.

- [ ] **Step 7: Clippy, fmt, commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`.

```bash
git add crates/ridl-ir/proto crates/ridl-sem/src/check.rs crates/ridl-ir/src \
  crates/ridl-diff/src crates/ridl-descriptor/tests crates/ridl-backend-rust/src \
  crates/ridl-backend-proto/tests crates/ridl-backend-flatbuffers/tests \
  crates/ridlc/tests crates/ridl/tests/facade.rs freeze/codegen-protocol.txt
git commit -m "feat(ridl-ir): add the interaction family and shape to the IR

Family and Shape are closed enums on SignalDef, EventDef, CommandDef
(shape too) and QueryDef, mirrored on the codegen model. The zero values
are dispatch and none, so no existing interaction encodes to a different
byte and the catalog hash of examples/cabin is unchanged; a test holds
it to freeze/wire.txt.

Refs driftsys/ridl#270.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 9: `check_interaction_class` in the readers and the writers

**Files:**

- Create: `crates/ridl-ir/src/class.rs`
- Modify: `crates/ridl-ir/src/lib.rs` (`pub mod class;` after `catalog_hash`;
  `from_json`, `from_binary`, `to_json_pretty`; `SerializeError`)
- Modify: `crates/ridl-ir/src/codegen.rs` (`from_json`, `from_binary`,
  `to_json_pretty`; `SerializeError`)
- Modify: `crates/ridl-ir/src/codegen/contract.rs:241-265` (`request_to_json`,
  `request_from_json`)
- Modify: `crates/ridlc/src/lib.rs:2425` (the `Emit::IrBinary` arm)

**Interfaces:**

- Consumes: the fields of Task 8.
- Produces, `pub` in `ridl_ir::class`:
  - `enum Kind { Signal, Event, Command, Query }`
  - `struct InteractionClassError { pub kind: Kind, pub family: i32, pub shape: i32 }`
    implementing `Display` and `std::error::Error`
  - `fn check_interaction_class(kind: Kind, family: i32, shape: i32) -> Result<(), InteractionClassError>`
  - `fn check_package(package: &v2::Package) -> Result<(), InteractionClassError>`
  - `fn check_model(model: &codegen::v1::Model) -> Result<(), InteractionClassError>`
  - a `Class(InteractionClassError)` variant on `v2::SerializeError` and on
    `codegen::SerializeError`.

- [ ] **Step 1: Create the module with a stub and the failing tests**

Write `crates/ridl-ir/src/class.rs`:

```rust
//! The valid (kind, family, shape) combinations of an interaction (ADR-0012
//! decision 4, as amended on 2026-10-11). The open cells of the decision's
//! table count as valid, so a later spelling for one needs no enum value.
//!
//! The IR readers call [`check_package`] on decode and the writers call it
//! before they encode, so an invalid combination such as
//! `(command, intent, none)` is refused where the IR is read, not by a lint a
//! caller can forget. The codegen readers and writers do the same with
//! [`check_model`].

use crate::codegen::v1;
use crate::v2;

/// The four kinds that carry a family. `fixed` carries none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Signal,
    Event,
    Command,
    Query,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Event => "event",
            Self::Command => "command",
            Self::Query => "query",
        }
    }
}

/// A (kind, family, shape) combination the table does not hold, or a family
/// or shape number outside its enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionClassError {
    pub kind: Kind,
    pub family: i32,
    pub shape: i32,
}

impl std::fmt::Display for InteractionClassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let family = v2::Family::try_from(self.family)
            .map_or_else(|_| format!("family {}", self.family), |family| family.as_str_name().to_string());
        let shape = v2::Shape::try_from(self.shape)
            .map_or_else(|_| format!("shape {}", self.shape), |shape| shape.as_str_name().to_string());
        write!(
            f,
            "invalid interaction class: a {} cannot be ({family}, {shape})",
            self.kind.word()
        )
    }
}

impl std::error::Error for InteractionClassError {}

/// Whether `(kind, family, shape)` is a cell of the table.
pub fn check_interaction_class(kind: Kind, family: i32, shape: i32) -> Result<(), InteractionClassError> {
    let _ = (kind, family, shape);
    Ok(())
}

/// Every interaction of every interface shape of `package`.
pub fn check_package(package: &v2::Package) -> Result<(), InteractionClassError> {
    for shape in package.shapes() {
        for decl in &shape.interface.interactions {
            check_decl(decl)?;
        }
    }
    Ok(())
}

fn check_decl(decl: &v2::Decl) -> Result<(), InteractionClassError> {
    use v2::decl::Kind as DeclKind;
    match decl.kind.as_ref() {
        Some(DeclKind::SignalDef(signal)) => check_interaction_class(Kind::Signal, signal.family, 0),
        Some(DeclKind::EventDef(event)) => check_interaction_class(Kind::Event, event.family, 0),
        Some(DeclKind::CommandDef(command)) => {
            check_interaction_class(Kind::Command, command.family, command.shape)
        }
        Some(DeclKind::QueryDef(query)) => check_interaction_class(Kind::Query, query.family, 0),
        _ => Ok(()),
    }
}

/// Every interaction of every interface of `model`.
pub fn check_model(model: &v1::Model) -> Result<(), InteractionClassError> {
    for interface in &model.interfaces {
        for interaction in &interface.interactions {
            let shape = match &interaction.shape {
                Some(v1::interaction::Shape::Command(command)) => command.shape,
                _ => 0,
            };
            let kind = match v1::Kind::try_from(interaction.kind) {
                Ok(v1::Kind::Signal) => Kind::Signal,
                Ok(v1::Kind::Event) => Kind::Event,
                Ok(v1::Kind::Command) => Kind::Command,
                Ok(v1::Kind::Query) => Kind::Query,
                _ => continue,
            };
            check_interaction_class(kind, interaction.family, shape)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2::{Family, Shape};

    fn ok(kind: Kind, family: Family, shape: Shape) -> bool {
        check_interaction_class(kind, family as i32, shape as i32).is_ok()
    }

    #[test]
    fn the_table_of_valid_combinations() {
        // ADR-0012 decision 4, the open cells counted as valid.
        for family in [Family::Dispatch, Family::Presentation, Family::Acquisition, Family::Control] {
            assert!(ok(Kind::Signal, family, Shape::None), "signal {family:?}");
        }
        assert!(!ok(Kind::Signal, Family::Intent, Shape::None));
        for family in [Family::Dispatch, Family::Presentation, Family::Intent, Family::Acquisition] {
            assert!(ok(Kind::Event, family, Shape::None), "event {family:?}");
        }
        assert!(!ok(Kind::Event, Family::Control, Shape::None));
        assert!(ok(Kind::Command, Family::Dispatch, Shape::None));
        assert!(ok(Kind::Command, Family::Control, Shape::None));
        for shape in [Shape::Activate, Shape::Toggle, Shape::Select, Shape::Adjust, Shape::Dismiss] {
            assert!(ok(Kind::Command, Family::Intent, shape), "command intent {shape:?}");
        }
        assert!(!ok(Kind::Command, Family::Intent, Shape::None), "decision 5: no generic intent");
        assert!(!ok(Kind::Command, Family::Dispatch, Shape::Toggle), "a shape needs intent");
        assert!(!ok(Kind::Command, Family::Presentation, Shape::None));
        assert!(!ok(Kind::Command, Family::Acquisition, Shape::None));
        for family in [Family::Dispatch, Family::Acquisition] {
            assert!(ok(Kind::Query, family, Shape::None), "query {family:?}");
        }
        assert!(!ok(Kind::Query, Family::Intent, Shape::None));
        assert!(!ok(Kind::Query, Family::Control, Shape::None));
    }

    #[test]
    fn a_number_outside_the_enums_is_invalid() {
        assert!(check_interaction_class(Kind::Signal, 99, 0).is_err());
        assert!(check_interaction_class(Kind::Command, 2, 99).is_err());
    }

    #[test]
    fn the_error_names_the_combination() {
        let err = InteractionClassError { kind: Kind::Command, family: 2, shape: 0 };
        assert_eq!(
            err.to_string(),
            "invalid interaction class: a command cannot be (FAMILY_INTENT, SHAPE_NONE)"
        );
    }
}
```

Add `pub mod class;` to `crates/ridl-ir/src/lib.rs` after
`pub mod catalog_hash;`. Check that `v2::InterfaceShape` exposes the interface
as `interface` (`lib.rs:~789`) and `v1::Interface` has `interactions`; adjust
the field access if the names differ.

- [ ] **Step 2: Run the tests to see the table test fail on its assertion**

Run: `cargo test -p ridl-ir --locked class::tests`

Expected: `the_table_of_valid_combinations` FAILS at
`assert!(!ok(Kind::Signal, Family::Intent, Shape::None))`;
`a_number_outside_the_enums_is_invalid` FAILS; the `Display` test passes.

- [ ] **Step 3: Implement the table**

Replace the stub body:

```rust
pub fn check_interaction_class(kind: Kind, family: i32, shape: i32) -> Result<(), InteractionClassError> {
    use v2::{Family, Shape};
    let error = || InteractionClassError { kind, family, shape };
    let family_value = Family::try_from(family).map_err(|_| error())?;
    let shape_value = Shape::try_from(shape).map_err(|_| error())?;
    let valid = match (kind, family_value, shape_value) {
        (Kind::Signal, Family::Dispatch | Family::Presentation | Family::Acquisition | Family::Control, Shape::None) => true,
        (Kind::Event, Family::Dispatch | Family::Presentation | Family::Intent | Family::Acquisition, Shape::None) => true,
        (Kind::Command, Family::Dispatch | Family::Control, Shape::None) => true,
        (Kind::Command, Family::Intent, Shape::Activate | Shape::Toggle | Shape::Select | Shape::Adjust | Shape::Dismiss) => true,
        (Kind::Query, Family::Dispatch | Family::Acquisition, Shape::None) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(error()) }
}
```

Run: `cargo test -p ridl-ir --locked class::tests`. Expected: PASS.

- [ ] **Step 4: Write the failing reader and writer tests**

In `crates/ridl-ir/src/lib.rs` tests (the `v2` tests module; find it with
`grep -n "mod tests" crates/ridl-ir/src/lib.rs`), add:

```rust
    /// A package with one interface `I` holding one command `c` of the given
    /// family and shape.
    fn package_with_command(family: Family, shape: Shape) -> Package {
        Package {
            name: "app".to_string(),
            interfaces: vec![Interface {
                name: "I".to_string(),
                visibility: Visibility::Public as i32,
                interactions: vec![Decl {
                    name: "c".to_string(),
                    visibility: Visibility::Public as i32,
                    ordinal: 1,
                    kind: Some(decl::Kind::CommandDef(CommandDef {
                        family: family as i32,
                        shape: shape as i32,
                        ..Default::default()
                    })),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn the_readers_reject_an_invalid_interaction_class() {
        // (command, intent, none) is written through the raw serializer,
        // which the check guards, and refused by both readers.
        let package = package_with_command(Family::Intent, Shape::None);
        let json = render_json(&package).unwrap();
        let err = from_json(&json).expect_err("the JSON reader must refuse it");
        assert!(err.to_string().contains("invalid interaction class"), "{err}");
        let bytes = prost::Message::encode_to_vec(&package);
        let err = from_binary(&bytes).expect_err("the binary reader must refuse it");
        assert!(err.to_string().contains("invalid interaction class"), "{err}");
    }

    #[test]
    fn the_json_writer_refuses_an_invalid_interaction_class() {
        let package = package_with_command(Family::Intent, Shape::None);
        let err = to_json_pretty(&package).expect_err("the writer must refuse it");
        assert!(matches!(err, SerializeError::Class(_)), "{err}");
    }

    #[test]
    fn a_valid_class_round_trips_through_both_readers() {
        let package = package_with_command(Family::Intent, Shape::Toggle);
        let json = to_json_pretty(&package).unwrap();
        assert_eq!(from_json(&json).unwrap(), package);
        assert_eq!(from_binary(&to_binary(&package)).unwrap(), package);
    }

    /// Review Focus 4: an IR written before `family` existed has no such key
    /// and reads as dispatch.
    #[test]
    fn a_snapshot_without_family_reads_as_dispatch() {
        let package = package_with_command(Family::Dispatch, Shape::None);
        let json = to_json_pretty(&package).unwrap();
        let stripped: String = json
            .lines()
            .filter(|line| !line.contains("\"family\"") && !line.contains("\"shape\""))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(stripped, json, "the rendering carried the two keys");
        let read = from_json(&stripped).expect("a 0.9 snapshot reads");
        assert_eq!(read, package);
    }
```

If `render_json` is `pub(crate)` it is reachable from the module's tests; if the
JSON of a command ends a line with a trailing comma issue after the strip, strip
the two keys with `serde_json::Value` instead (remove `family` and `shape` from
the command object and re-render).

In `crates/ridl-ir/src/codegen/tests.rs` (Review Focus 5), add:

```rust
#[test]
fn the_lenient_request_reader_rejects_an_invalid_class() {
    let mut request = v1::CodegenRequest::default();
    let mut model = v1::Model::default();
    model.interfaces.push(v1::Interface {
        interactions: vec![v1::Interaction {
            kind: v1::Kind::Command as i32,
            family: v1::Family::Intent as i32,
            shape: Some(v1::interaction::Shape::Command(v1::CommandShape {
                shape: v1::Shape::None as i32,
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    });
    request.model = Some(model);
    let json = crate::v2::render_json(&request).unwrap();
    let err = crate::codegen::request_from_json(&json).expect_err("the lenient reader must refuse it");
    assert!(err.to_string().contains("invalid interaction class"), "{err}");
    let err = crate::codegen::request_to_json(&request).expect_err("the writer must refuse it");
    assert!(matches!(err, crate::codegen::SerializeError::Class(_)), "{err}");
}
```

Read `v1::CodegenRequest` and `v1::Interface` for the field names (`model`,
`interactions`) and adjust.

- [ ] **Step 5: Run them to see them fail on their assertions**

Run:
`cargo test -p ridl-ir --locked interaction_class reads_as_dispatch round_trips`

Expected: the reject tests FAIL on `expect_err` (the readers accept the
package); `a_valid_class_round_trips_through_both_readers` and
`a_snapshot_without_family_reads_as_dispatch` PASS.

- [ ] **Step 6: Call the check in the readers and the writers**

In `crates/ridl-ir/src/lib.rs`:

- Add to `SerializeError`:
  `/// An interaction holds a (kind, family, shape) combination the IR does not admit (ridl_ir::class).`
  `Class(crate::class::InteractionClassError),` and the matching `Display` arm
  (`write!(f, "{err}")`) and `source` arm if the impl has one.
- `to_json_pretty`:
  `crate::class::check_package(package).map_err(SerializeError::Class)?;` before
  `render_json(package)`.
- `from_json`:
  `let package: Package = read_json(text)?; crate::class::check_package(&package).map_err(serde::de::Error::custom)?; Ok(package)`.
- `from_binary`:
  `let package: Package = prost::Message::decode(bytes)?; crate::class::check_package(&package).map_err(|err| prost::DecodeError::new(err.to_string()))?; Ok(package)`.
- `to_binary` keeps its signature (it cannot fail); the one caller that writes
  an artifact with it, `ridlc`, checks first (below).

In `crates/ridl-ir/src/codegen.rs`: the same three edits on the model's
`to_json_pretty`, `from_json`, `from_binary` with `crate::class::check_model`,
and the `Class` variant on its `SerializeError` (with `from_v2` mapping
`v2::SerializeError::Class(err)` to `Self::Class(err)`).

In `crates/ridl-ir/src/codegen/contract.rs`: `request_to_json` checks
`request.model` with `check_model` (when present) before rendering;
`request_from_json` reads, then checks the model the same way, mapping the error
with `serde::de::Error::custom`.

In `crates/ridlc/src/lib.rs`, in the `Emit::IrBinary` arm, before
`std::fs::write`:
`if let Err(err) = ridl_ir::class::check_package(ir) { diagnostics.push(error_diagnostic("", err.to_string(), FileId::DETACHED, TextRange::default())); continue; }`
— the same reporting the `Emit::IrJson` arm uses for a render error.

Update the rustdoc of `from_json` (the paragraph that lists what the strict
reader rejects) with one sentence: "After the parse, every interaction's (kind,
family, shape) is checked against `ridl_ir::class`; an invalid combination is an
error of this reader."

- [ ] **Step 7: Run the crate tests, the workspace tests, clippy, fmt**

Run: `cargo test -p ridl-ir --locked`, `cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`.

Expected: PASS. No dump changes (`git status --short freeze/` empty).

- [ ] **Step 8: Commit**

```bash
git add crates/ridl-ir/src/class.rs crates/ridl-ir/src/lib.rs crates/ridl-ir/src/codegen.rs \
  crates/ridl-ir/src/codegen/contract.rs crates/ridl-ir/src/codegen/tests.rs crates/ridlc/src/lib.rs
git commit -m "feat(ridl-ir): check the interaction class where the IR is read and written

ridl_ir::class holds the valid (kind, family, shape) table of ADR-0012
decision 4, with the open cells valid. The JSON and binary readers of
the package and of the codegen model, the lenient request reader, and
the writers refuse a combination outside it, so (command, intent, none)
is rejected where the IR is read.

Refs driftsys/ridl#270.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

### Task 10: `InteractionClassChanged`, the ADR amendment and the plugin heads-up

**Files:**

- Modify: `crates/ridl-diff/src/lib.rs:124-240` (the `Category` enum),
  `:540-560` (`category_word`), `:595-640` (the exhaustive match that follows)
- Modify: `crates/ridl-diff/src/classify.rs:95-140` (`classify`), `:1089-1340`
  (`explain`)
- Modify: `crates/ridl-diff/src/walk.rs:890-1030` (`diff_interaction`)
- Modify: `crates/ridl-diff/src/classify/classify_tests.rs` (one test)
- Modify: `docs/book/cli-reference.md:1625-1655` (the category list the test
  `crates/ridl/tests/cli_reference.rs` compares with `ridl diff --explain`)
- Modify: `docs/decisions/ADR-0012-interaction-boundary-model.md` (`## Status`,
  decision 6), `docs/specification/ridl-family-overview.md` (§5, one row)

**Interfaces:**

- Consumes: `SignalDef.family` and the other fields of Task 8.
- Produces: `Category::InteractionClassChanged`, word
  `interaction_class_changed`, verdict `Breaking` in both directions.

- [ ] **Step 1: Write the failing classification test**

In `crates/ridl-diff/src/classify/classify_tests.rs`, after
`a_require_removed_is_compatible`'s group, add:

```rust
// ==========================================================================
// Family and shape.
// ==========================================================================

fn with_class(family: v2::Family, shape: v2::Shape) -> v2::Package {
    let mut package = pkg(vec![command("c", 1, vec![], vec![])]);
    let Some(v2::decl::Kind::CommandDef(def)) = package.interfaces[0].interactions[0].kind.as_mut()
    else {
        unreachable!("the builder wrote a command");
    };
    def.family = family as i32;
    def.shape = shape as i32;
    package
}

/// A family or shape change on a name-paired interaction is breaking in both
/// directions (ADR-0012 decision 12: evolution rules are uniform). No source
/// can write a family other than dispatch yet; the walk compares the fields
/// from the start so the diff fails closed.
#[test]
fn a_family_or_shape_change_is_breaking_both_ways() {
    let dispatch = with_class(v2::Family::Dispatch, v2::Shape::None);
    let control = with_class(v2::Family::Control, v2::Shape::None);
    assert_row(&dispatch, &control, Category::InteractionClassChanged, Verdict::Breaking);
    assert_row(&control, &dispatch, Category::InteractionClassChanged, Verdict::Breaking);
    let toggle = with_class(v2::Family::Intent, v2::Shape::Toggle);
    let select = with_class(v2::Family::Intent, v2::Shape::Select);
    assert_row(&toggle, &select, Category::InteractionClassChanged, Verdict::Breaking);
    let report = diff_packages(&toggle, &select);
    let change = report
        .changes
        .iter()
        .find(|change| change.category == Category::InteractionClassChanged)
        .expect("the change is listed");
    assert_eq!(change.before.as_deref(), Some("intent toggle"));
    assert_eq!(change.after.as_deref(), Some("intent select"));
}
```

Add the variant so the test compiles, in the `Category` enum after
`ContractChanged`:

```rust
/// A name-paired interaction's family or operation shape changed
/// (ADR-0012 decision 6).
InteractionClassChanged,
```

and the minimal arms the three exhaustive matches need so that the crate
compiles with the walk not yet comparing the fields: in `category_word`,
`Category::InteractionClassChanged => "interaction_class_changed",`; in the
match at `lib.rs:~611`, add the variant to the `=> None` arm list; in
`classify`, `Category::InteractionClassChanged => Verdict::Compatible,`
(deliberately wrong, replaced in Step 3); in `explain`, a row whose text is
`"placeholder"` (replaced in Step 3).

- [ ] **Step 2: Run the test to see it fail on its assertion**

Run:
`cargo test -p ridl-diff --locked a_family_or_shape_change_is_breaking_both_ways`

Expected: FAIL in `assert_row`'s `row(&report, category)` (no change of that
category is listed), or, if `row` panics with its own message, on that message.
`every_category_has_a_rule_row_naming_its_verdicts` also FAILS on the
placeholder row.

- [ ] **Step 3: Compare the fields in the walk, classify and explain**

In `crates/ridl-diff/src/walk.rs`, add the helper:

```rust
/// `dispatch`, `control`, `intent toggle`: the family, then the shape when
/// one is set.
fn class_str(family: i32, shape: i32) -> String {
    let family = v2::Family::try_from(family)
        .map_or_else(|_| family.to_string(), |family| family.as_str_name().trim_start_matches("FAMILY_").to_ascii_lowercase());
    match v2::Shape::try_from(shape) {
        Ok(v2::Shape::None) | Err(_) => family,
        Ok(shape) => format!(
            "{family} {}",
            shape.as_str_name().trim_start_matches("SHAPE_").to_ascii_lowercase()
        ),
    }
}
```

In `diff_interaction`, in each of the four kind arms, add before the
kind-specific comparisons (for a command, `a.shape`/`b.shape`; for the others,
`0`):

```rust
if a.family != b.family || a.shape != b.shape {
    emit(
        changes,
        path.to_string(),
        Category::InteractionClassChanged,
        Some(class_str(a.family, a.shape)),
        Some(class_str(b.family, b.shape)),
    );
}
```

(for signal, event and query: `if a.family != b.family` with
`class_str(a.family, 0)`.) If `without_member_docs` rebuilds the kind, confirm
it copies `family` and `shape` (a `..def.clone()` or a full literal; Task 8's
sweep covered it).

In `classify.rs`, replace the placeholder arm with
`Category::InteractionClassChanged => Verdict::Breaking,` under the comment
`// A family or shape is part of the contract a consumer compiled against; a change moves it in both directions (ADR-0012 decision 12).`
Replace the placeholder explain row with:

```rust
Category::InteractionClassChanged => concat!(
    "A name-paired interaction's family or operation shape changed\n",
    "(ADR-0012 d6).\n",
    "  breaking    any change, in either direction: the family selects the\n",
    "              obligations a consumer compiled against, and the shape\n",
    "              the gesture an intent command is. No source writes a\n",
    "              family other than dispatch yet; the walk compares the\n",
    "              fields from the start so the diff fails closed (ADR-0012 d9)"
),
```

- [ ] **Step 4: Update the category list the book quotes**

In `docs/book/cli-reference.md`, in the `ridl diff --explain` listing, insert
`interaction_class_changed` after `contract_changed` (the position `CATEGORIES`
gives it: declaration order).

- [ ] **Step 5: Run the diff crate, the CLI reference test, clippy, fmt**

Run: `cargo test -p ridl-diff --locked`,
`cargo test -p ridl --locked --test cli_reference`,
`cargo test -p ridl-mcp --locked` (`ridl_explain` reads `ridl_diff::explain`),
`cargo clippy -p ridl-diff -p ridl -p ridl-mcp --all-targets -- -D warnings`,
`cargo fmt --all --check`, `cargo xtask freeze check` (the `mcp` dump holds no
category list; expected unchanged).

Expected: PASS.

- [ ] **Step 6: Amend ADR-0012 decision 6 and add the ledger row**

Under `## Status`, after Task 7's amendment, insert:

```markdown
**Amendment, 2026-10-11 — the two enums as built (decision 6).** `Family` and
`Shape` are enums of `ir.proto`, mirrored in the codegen model. `family` is a
field of `SignalDef`, `EventDef`, `CommandDef` and `QueryDef`; `FixedDef` has
none, because `fixed` is family-neutral (decision 4). `shape` is a field of
`CommandDef` only, so a shaped signal, event or query cannot be expressed; in
the codegen model it sits on `CommandShape`, because `Interaction`'s oneof is
named `shape`. The zero values are `FAMILY_DISPATCH` and `SHAPE_NONE`, a
departure from the `_UNSPECIFIED = 0` convention of the schema, so that every
existing interaction encodes to the same bytes, its catalog hash does not
change, and an IR written before the fields decodes correctly. The valid
(kind, family, shape) combinations are one table, `ridl_ir::class`, with the
open cells of decision 4 counted as valid; the IR readers and writers call it,
so `(command, intent, none)` is refused where the IR is read, as decision 6
asks. `ridl diff` reports a family or shape change as
`interaction_class_changed`, breaking. Recorded under decision 6. Issue
driftsys/ridl#270.
```

Append to decision 6:

```markdown
_Amended 2026-10-11:_ built. `family` on the four interaction kinds,
`shape` on the command only, zero values dispatch and none, the valid
combinations in `ridl_ir::class`, the diff category
`interaction_class_changed`.
```

Add ledger row 41 to the overview:

```markdown
| 41 | **Family and shape in the IR**: `Family` and `Shape` enums with their full value sets, zero values dispatch and none so no catalog hash changes; `ridl_ir::class` holds the valid combinations and the readers refuse the rest; `interaction_class_changed` is breaking (driftsys/ridl#270) | ADR-0012 decision 6, IR `ir.proto` |
```

Run:
`prim fmt docs/decisions/ADR-0012-interaction-boundary-model.md docs/specification/ridl-family-overview.md docs/book/cli-reference.md`,
`just check`, `just link-check`, `just story-id-check`.

- [ ] **Step 7: Commit**

```bash
git add crates/ridl-diff/src docs/book/cli-reference.md \
  docs/decisions/ADR-0012-interaction-boundary-model.md docs/specification/ridl-family-overview.md
git commit -m "feat(ridl-diff): report a family or shape change as breaking

InteractionClassChanged is emitted for a name-paired interaction whose
family or shape differs, and classified breaking in both directions.
ADR-0012 decision 6 records the placement and the zero values.

Closes driftsys/ridl#270.

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 8: Run the full gate, open the pull request, file the heads-up**

Run: `just build`. Expected: PASS. Rebase onto `main`, run
`cargo xtask freeze update`, confirm only `freeze/codegen-protocol.txt` differs
from `main` and only by added lines, then open the pull request from
`feat/interaction-family` with `Closes driftsys/ridl#270` in its body.

After it merges, file the heads-up in the Kotlin plugin's tracker:

```bash
gh issue create --repo driftsys/ridlc-gen-kotlin \
  --title "ridl 0.10: Interaction.family and CommandShape.shape in the codegen model" \
  --body "ridl 0.10 adds two fields to ridl.codegen.v1: Interaction.family (enum Family, zero value FAMILY_DISPATCH) and CommandShape.shape (enum Shape, zero value SHAPE_NONE). Every interaction ridl emits today carries the zero values. The lenient request reader drops a key it does not know, so a plugin built against an older ridl-ir keeps working and sees the core kind (ADR-0012 decision 7). A plugin that reads family can select on it; nothing is required. Source: driftsys/ridl#270 and docs/decisions/ADR-0012-interaction-boundary-model.md, amendment of 2026-10-11."
```

---

## Self-review

**Spec coverage.** Section 3.1 (16 words leave, `states`/`during` stay, no
profile gains a word) — Task 1. Section 3.2 (the member-start rule, the ROS 2
evidence) — Task 2 prose, Task 1's pin test. Section 3.3 (typl §1.4, ridl §2.3,
ADR-0012 under decision 4, `freeze/keywords.txt`) — Tasks 1 to 3. Section 4.1
(the table and its rows) — Task 4. Section 4.2 (the readers, the deleted lists,
the unchanged diagnostics, the system diff) — Tasks 5 and 6. Section 4.3 tests 1
to 4 — Task 4 (1, 2) and Task 6 (3, 4). Section 4.4 (the dump) — Task 7. Section
4.5 (ridl and rsdl references, ADR-0012 decisions 8 and 9) — Task 7. Section 5.1
(the enums, the D-6 comment) — Task 8. Section 5.2 (placement) — Task 8, with
the `CommandShape` deviation flagged in the PR 3 preamble. Section 5.3
(`check_interaction_class`, the readers and writers) — Task 9. Section 5.4
(lowering, the hash test, `InteractionClassChanged`) — Tasks 8 and 10. Section
5.5 (lenient plugins, the protocol dump, the Kotlin heads-up) — Tasks 8, 9 and
10. Section 5.6 (ADR-0012 decision 6) — Task 10. Section 6 (three pull requests,
rebase and `freeze update`, no diagnostic change, the mutation checks for
reviewers) — the three preambles and the Global Constraints; the reviewer
mutations named in section 6 (delete a `Compared` arm, accept
`(command, intent, none)`) are caught by Task 6's registry test and Task 9's
table test.

**Where the code contradicts the spec.** (1) `tier` and `deprecated` are not in
the system artifact today, so Task 6 lowers them before the diff can report
them. (2) The codegen model's `Interaction` already has a `oneof shape`, so Task
8 puts `shape` on `CommandShape`; this needs Sebastien's confirmation. (3) The
spec's `Compared(heading)` has no single heading for `labels`, `deprecated` and
the backend pattern, which sit on composition and placement sites both; the
`Diff::Compared` text names both headings. (4) A distribution added or removed
is not a composition change today and this plan does not make it one; only
name-paired distributions are compared.

**Placeholder scan.** No "TBD", no "similar to Task N", no step without its
code. Three steps direct the implementer to read a neighbouring definition for
an exact name (`PRELUDE`'s type, `push_values`'s words, `CodegenRequest`'s
fields) and say what to do in each case.

**Type consistency.**
`ridl_core::attributes::{AttrKey, AttrName, Owner, Form, Site, Diff, NotReported, ATTRIBUTES, lookup, backend_pattern}`
are the names Tasks 4, 5, 6 and 7 use. `AttrKey::key()`,
`AttrKey::allows(Site)`, `Form::word()`, `Site::word()` match between Task 4 and
Task 7. `v2::Tier`, `Distribution.tier`, `deprecated: Option<String>` match
between Task 6's proto, lowering and tests. `v2::Family::Dispatch`,
`v2::Shape::None`,
`ridl_ir::class::{Kind, InteractionClassError, check_interaction_class, check_package, check_model}`
and `SerializeError::Class` match between Tasks 8, 9 and 10.

**Review Focus.** Five lines, each with its test in the owning task: 1 in Task
1, 2 in Task 5, 3 in Task 6, 4 and 5 in Task 9.
