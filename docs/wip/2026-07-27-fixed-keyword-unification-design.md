# Design — unify the provisioned constant on `fixed`

- Date: 2026-07-27
- Status: design approved, not yet implemented

## 1. The question

ridl spells the provisioned constant `final`; uxdl spells the same primitive
`fixed`. General-form §6.5 reopened the naming, ADR-0008 decision 5 froze
`final` for the duration of epic E2, and E2 is now closed. The question is which
word the family uses.

`final` misleads. A Java or Kotlin reader takes it for a compile-time constant.
The actual semantics are: provisioned externally — build, factory, or
over-the-air update — and immutable for the lifetime of the running software
instance. A later software instance may hold a different value. That is a
different notion from typl's `const`, which is baked into the artifact and
identical in every instance.

## 2. Decision

ridl's `final` becomes `fixed`. uxdl is unchanged.

## 3. Rationale

### 3.1 Why not keep two spellings

Per-profile vocabulary is the family's systematic design, not an accident: the
interact core table gives every primitive two spellings — `signal`/`display`,
`event`/`input`, `command`/`action`, `query`/`fetch`. Each of those pairs earns
its divergence, because each uxdl section adds genuine profile-specific rules: a
display carries provenance and a skeleton state, an input carries gesture time
and an occurrence TTL, an action carries refinements, a fetch carries UXDL-206.

uxdl §8 adds none. It is one sentence — "the uxdl profile of `final`. All of
ridl §8 applies" — and it is the only section in that document that delegates
wholesale. The row with no profile-specific semantics is the row that does not
need profile-specific words.

### 3.2 Why `fixed` and not the alternatives

| Candidate     | Verdict  | Reason                                                                                                         |
| ------------- | -------- | -------------------------------------------------------------------------------------------------------------- |
| `fixed`       | chosen   | already in the registry as uxdl's word; costs zero new entries and removes one; no compile-time-constant prior |
| `final`       | rejected | the Java and Kotlin prior points at compile-time constant, which is the wrong row of the table                 |
| `value`       | rejected | the general form calls shape 1 the value declaration; the word names the shape, not the kind                   |
| `option`      | rejected | typl owns `?` optionality, and configuration vocabulary reads "option" as a settable preference                |
| `preset`      | rejected | consumer-electronics presets are user-changeable and re-selectable, which inverts the semantics                |
| `static`      | rejected | the family already uses "static" for wire posture — static bus against discovered service                      |
| `frozen`      | rejected | `ridlc --frozen` is the lockfile-pinning flag, MANI-103 and MANI-104                                           |
| `constant`    | rejected | one letter from typl's `const`, for a different concept                                                        |
| `provisioned` | rejected | names the origin but not the immutability, and is long                                                         |

The cost of `fixed` is prose, not semantics: typl documents use "fixed" as an
ordinary English adjective for array sizes ("fixed array", "fixed 8 bytes").
Those become "exact-length" where the collision is confusing.

## 4. Scope

### 4.1 Surface and registry

`fixed` moves from a uxdl-only registry entry to a shared entry, the way
`signal` and `event` are shared across ridl, rmdl, and rsdl — one registry entry
per concept. In `crates/ridl-syntax/src/keywords.rs`, `fixed` joins the ridl
keyword table and `FinalKw` becomes `FixedKw`.

`final` leaves the reserved-word registry entirely. It is not retained as a
retired-but-reserved word. Two reasons: typl set the precedent when it retired
`default` without reserving it, and the registry admission test requires every
entry to name a describable property, which a retired keyword does not. The
transition population is zero (§4.2), so the migration diagnostic a reserved
`final` would buy has no one to serve.

### 4.2 IR v2

`FinalDef` becomes `FixedDef` and `final_def` becomes `fixed_def`. **Field
number 20 does not change** — the number is the compatibility contract under
ADR-0008 decision 8, the name is not.

The oneof serializes by variant name, so the `.ir.json` key `"FinalDef"` becomes
`"FixedDef"`. A baseline snapshot written by the current build would fail to
deserialize under the new one, because serde rejects unknown enum variants. That
is the correct failure mode for a breaking-change gate: loud, not a silent drop
of the affected interactions.

No migration shim and no deprecation window are needed. The workspace version is
`0.0.0`, the repository has no release tags, and `ridl` and `ridl-lsp` are
`publish = false`. No baseline exists outside the repository's own fixtures, so
the in-repository snapshots simply regenerate.

### 4.3 Diagnostics

RIDL-106, RIDL-301, and FORM-102 keep their codes. Only their message text
changes. Diagnostic codes are the stability contract; message text is not.

The rename also closes a defect found while surveying this question: uxdl
Appendix C's `fixed_def` production references `final_type`, a nonterminal
defined only in ridl's Appendix C and absent from the borrowed-production list
in uxdl's Appendix C preamble. The production becomes `fixed_type`, and
`fixed_type` is added to that borrowed list.

### 4.4 Code

298 matching lines across ten crates: `ridl-syntax` (keyword table,
`SyntaxKind`, AST, parser), `ridl-sem`, `ridl-ir`, `ridl-backend-rust`,
`ridl-backend-ts`, `ridl-lsp`, `ridlc`, `ridl-diff` (including the literal
`"final"` kind label in `walk.rs`), `ridl-core`, and `xtask`. The change is
mechanical. It also renames the `finals_reserved.ridl` fixture and regenerates
every insta snapshot that holds a lowered final.

Outside the Rust workspace, `editors/vscode/syntaxes/ridl.tmLanguage.json`
carries the keyword list in both its header comment and its keyword match
pattern. No gate covers that file, so it is easy to miss.

### 4.5 Documents

About 90 occurrences. The load-bearing ones:

- ridl reference — §8 and its heading, the five-kinds table, the glossary entry,
  Appendix B, Appendix C
- typl reference — §1.3, the §1.4 keyword registry, the Appendix F mapping row
- uxdl reference — the interact core table, §8, Appendix C
- overview — decision-ledger item 6
- general form — §6.5 closed, plus the shape-1 and summary tables
- ROADMAP — the open-item line naming the reconsideration
- book — `getting-started.md` holds 39, many inside fenced blocks that
  `crates/ridl/tests/book_examples.rs` compiles, so the gate fails if prose and
  keyword drift apart

The interact core table keeps the provisioned-constant row, with both cells
reading `fixed`. The table is the map of the core primitives; dropping the row
would hide one.

### 4.6 Where the decision is recorded

A new **ADR-0011**, following the ADR-0009 and ADR-0010 pattern: not
epic-scoped, binding until superseded. ADR-0008 decision 5 receives a
superseding pointer rather than a rewrite, and editing that file requires the
whole-document sweep its own editing note demands — run against the file, not
against the diff.

## 5. Non-goals

- No change to the semantics of the primitive. Only the keyword changes.
- No change to any other interact-core keyword.
- No change to IR field numbers, diagnostic codes, or ordinal derivation.
- No migration tooling, deprecation shim, or reserved-word alias for `final`.

## 6. Success criteria

1. `just build` passes — this covers the compiled book examples, the workspace
   test suite, clippy, formatting, and the lint gate.
2. Over `crates/`, `xtask/`, `editors/`, and the live document trees
   (`docs/specification/`, `docs/book/`, `docs/wip/`, `docs/technotes/`),
   `grep -rniE '\bfinal\b'` returns only occurrences where "final" is ordinary
   English, and none where it names the keyword, the IR message, or the syntax
   kind. `docs/archive/` is excluded: archived plans and the v0.1 reference are
   verbatim history and are not rewritten. `docs/decisions/` is excluded for the
   same reason — an ADR recording a past decision about the word `final` keeps
   its text, and ADR-0008 decision 5 gains only the superseding pointer.
3. The `family_registry_is_consistent` test in `ridl-syntax` passes with `fixed`
   as an active ridl keyword and `final` absent from the registry.
4. General-form §6.5 and overview ledger item 6 record the decision as closed,
   and ADR-0011 exists.

## 7. Risks

- **Snapshot churn hides a real change.** The regeneration touches many golden
  files. Mitigation: review the regenerated diff for any change that is not a
  `final` to `fixed` substitution before committing.
- **A falsified sentence survives in ADR-0008.** That document has a recorded
  history of exactly this failure. Mitigation is the sweep required by §4.6.
- **Prose collision with "fixed" as an English adjective.** Mitigation: reword
  to "exact-length" where a reader could take the adjective for the keyword.
