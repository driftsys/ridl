# ADR-0011 — The provisioned-constant keyword is `fixed`

## Status

Accepted — 2026-07-27. Scope: the surface keyword naming the provisioned
constant in ridl and uxdl, and the registry, IR, and diagnostic consequences of
renaming it. Not epic-scoped: it binds the language surface until superseded,
the way ADR-0009 binds the gate and ADR-0010 binds the CLI contract. It follows
the ADR-0006 / ADR-0007 / ADR-0008 / ADR-0009 / ADR-0010 pattern of recording an
agent-taken decision for after-the-fact maintainer review. It supersedes
ADR-0008 decision 5, which froze the earlier spelling for the duration of epic
E2.

## Context

ridl spelled the provisioned constant `final`; uxdl spelled the same primitive
`fixed`. General-form §6.5 reopened the naming and left it undecided; ADR-0008
decision 5 froze `final` for the duration of E2 on the grounds that the
published reference used it decisively. E2 is closed, so nothing binds the
answer any longer.

`final` misleads. A Java or Kotlin reader takes it for a compile-time constant.
The semantics are different in a way that matters at a contract boundary: the
value is provisioned externally — build, factory, or over-the-air update — and
is immutable for the lifetime of the **running software instance**. A later
instance of the same software may hold a different value. That is a distinct
notion from typl's `const`, which is baked into the artifact and identical in
every instance, and from a persisted signal, which changes while the system
runs.

The obvious objection to unifying the two profiles is that per-profile
vocabulary is the family's systematic design, not an accident. The interact core
gives every primitive two spellings: `signal`/`display`, `event`/`input`,
`command`/`action`, `query`/`fetch`. Unifying one row makes that row the
exception, and an exception needs a reason.

The reason is that the other rows earn their divergence and this one does not.
Each uxdl section adds genuine profile-specific rules — a display carries
provenance and a skeleton state, an input carries gesture time and an occurrence
TTL, an action carries the five refinements, a fetch carries UXDL-206. uxdl §8
adds none. Before this change it read, in full: "A **static capability** — the
uxdl profile of `final`. All of ridl §8 applies." It is the only section in that
document that delegates wholesale. The row with no profile-specific semantics is
the row that does not need profile-specific words.

## Decision

1. **ridl's `final` is renamed to `fixed`; uxdl is unchanged.** Both profiles
   spell the provisioned constant `fixed`, which is one registry entry for one
   concept — the same treatment `signal` and `event` already get where rmdl and
   rsdl reuse them.

2. **`final` is removed from the family reserved-word registry entirely.** It is
   not retained as a retired-but-reserved word, and no alias, deprecation shim,
   or migration diagnostic is added.

   **This is the first retraction of an implemented keyword from the registry,
   and it has no true precedent.** typl §1.4 records that `default` was retired
   without being reserved, but `default` was never in the implemented registry —
   a pickaxe search of the whole history for `DefaultKw`, and of
   `crates/ridl-syntax/src/keywords.rs` for the string, both come back empty. It
   was a design-time rejection. `final` spent two epics as an active keyword
   with three diagnostics, an IR field, and corpus coverage, so the analogy is
   directional only: it shows the registry does not keep retired words, not that
   retracting a shipped one is routine.

   The removal is taken on the narrower ground that the migration population is
   empty: the workspace version is `0.0.0`, the repository has no release tags,
   and the binaries are `publish = false`, so no source outside this repository
   uses the old word and a reserved `final` would produce a better error message
   for nobody. A future retraction of a keyword that has shipped to real
   consumers should not cite this decision as precedent without re-examining
   that ground.

3. **IR field number 20 is unchanged; only the message and field names move.**
   `FinalDef` becomes `FixedDef` and `final_def` becomes `fixed_def`. The number
   is the compatibility contract under ADR-0008 decision 8; the name is not.

   The `.ir.json` surface does change, because the oneof serializes by variant
   name — the key `"FinalDef"` becomes `"FixedDef"`. A baseline snapshot written
   before this change fails to deserialize after it, because serde rejects
   unknown enum variants. That is the correct failure mode for a breaking-change
   gate: loud, rather than a silent drop of the affected interactions from the
   comparison. It costs nothing in practice, for the same reason decision 2
   needs no migration path.

4. **Diagnostic codes are unchanged.** RIDL-106, RIDL-301, and FORM-102 keep
   their identifiers; only their message text moves. Codes are the stability
   contract that ADR-0008 decision 9 and ADR-0010 rely on. Message text is not.

5. **The interact-core table keeps the provisioned-constant row, with both cells
   reading `fixed`.** The table is the map of the core primitives. Collapsing
   the row would hide one.

## Alternatives considered

| Candidate     | Verdict  | Reason                                                                                                            |
| ------------- | -------- | ----------------------------------------------------------------------------------------------------------------- |
| `fixed`       | chosen   | already in the registry as uxdl's word; costs zero new entries and removes one; no compile-time-constant prior    |
| `final`       | rejected | the Java and Kotlin prior points at a compile-time constant, which is the wrong notion                            |
| `value`       | rejected | the general form calls shape 1 the value declaration; the word names the declaration shape, not the kind          |
| `option`      | rejected | typl owns `?` optionality, and configuration vocabulary reads "option" as a settable preference                   |
| `preset`      | rejected | consumer-electronics presets are user-changeable and re-selectable, which inverts the semantics                   |
| `static`      | rejected | the family already uses "static" for wire posture — a static bus against a discovered service (ridl §17, rsdl §8) |
| `frozen`      | rejected | `ridlc --frozen` is the lockfile-pinning flag (MANI-103, MANI-104)                                                |
| `constant`    | rejected | one letter away from typl's `const`, for a different concept                                                      |
| `provisioned` | rejected | names the origin but not the immutability, and is long                                                            |

## Consequences

- Positive: one word for one concept across both boundaries, and the misreading
  that prompted general-form §6.5 is gone from keyword position — the last
  bullet below records where it survives. General-form open question 6 is
  closed; the open-question list renumbers.
- Positive: uxdl Appendix C's `fixed_def` production had referenced
  `final_type`, a nonterminal that only ridl's appendix defined and that uxdl's
  borrowed-production list did not name. The rename made the dangle visible and
  closed it.
- Positive: the UXDL-105 row no longer lists this keyword among the
  system-interaction words a `.uxdl` file rejects, because a `.uxdl` file
  accepts it.
- Negative: the rename touched ten crates, the generated AST, twenty-two
  snapshots, every `.ridl` fixture and corpus member, the compiled book
  examples, and the VS Code TextMate grammar. It is the largest mechanical
  change since E2 closed.
- Negative: "fixed" is also an ordinary English adjective for array and string
  lengths, and typl's tables used it that way. Those four rows were reworded to
  "exactly N" and "exact-length". The collision is not fully eliminated — it is
  moved out of the places where a reader could take the adjective for the
  keyword.
- Negative: because decision 2 removes `final` from the registry rather than
  retaining it as reserved, `final` is now a legal identifier: a signal may be
  named `final`, and so may a struct field. The misreading this ADR exists to
  eliminate is therefore not gone — it is moved from keyword position to
  identifier position, where no diagnostic flags it. Accepted knowingly: an
  author who names a signal `final` has written a confusing name, which is a
  naming-guidance matter (ridl §15), not a language-surface one, and reserving a
  word forever to prevent one bad identifier is a worse trade.
- Neutral: nothing outside this repository consumed the old keyword or the old
  IR key, so no migration path exists and none is owed.

## References

- `docs/wip/family-general-form.md` §6.5 — the reopened question this closes
- ADR-0008 decision 5 — the E2 freeze this supersedes
- ADR-0008 decision 8 — the IR field-numbering contract decision 3 preserves
- `docs/specification/ridl-language-reference.md` §8 — the kind as specified
- `docs/specification/uxdl-language-reference.md` §1.2, §8 — the interact core
  table and the uxdl profile of this kind
- `docs/specification/typl-language-reference.md` §1.4 — the reserved-word
  registry
