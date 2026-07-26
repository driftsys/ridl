# ADR-0008: Epic E2 Execution Decisions (ridl — the Interface Layer)

## Status

Accepted (agent-taken, maintainer-reviewable); decisions 15 to 21 are later
amendments, each dated in its own text. Each numbered decision below was taken
to unblock the epic E2 execution plan, which lives at
`docs/archive/2026-07-19-e2-ridl-interface-layer-plan.md` (moved from
`docs/wip/` at epic close), and is reversible at the cost of a small refactor
before a later epic builds on it. This ADR follows the pattern of ADR-0006 (E0)
and ADR-0007 (E1).

**Editing note.** Revisions to this document have repeatedly left behind a
sentence describing the state they replaced — seventeen found and corrected so
far. They were scattered rather than clustered: a decision's lead contradicting
its own body, the Status line above, a scoping claim in `## Consequences`, and a
closed enumeration in an earlier decision that a later one silently extended.
Several sat in sections the revision that falsified them never opened, so
reading the diff does not find them. **Sweeping the whole document for sentences
the edit has falsified is a precondition for editing it here, not optional
diligence.**

_Extended (2026-07-26)._ The count above stood at six until this sweep, and the
sixth was found during review of the note that said five. The whole-document
sweep this note demands was then run for the first time — against the repository
at `af7ef7c`, not against a diff — and found **ten** more.

The tenth arrived the way the sixth did. Review of the sweep that reported nine
found it in **decision 1**, whose "the ridl reference text is stale on these
four" the same sweep's own new `## Consequences` note contradicted in the same
commit — a correction stranding a sentence in the document it was correcting.
Three of the seventeen have now been found by reviewing a correction rather than
by making one: the sixth, the sixteenth, and the seventeenth.

Two of the ten were already recorded as issues: decision 13's allocation ledger
(#169) and decision 17's "those are the only sites" claim (logged in #172). The
other eight nothing had recorded — four in decision 21, one in decision 20, two
in `## Consequences`, and decision 1's. Seven PRs merged between the amendments
landing (#165) and this sweep, and every one of those eight was stranded by a PR
that never opened this file. That is the mechanism this note describes,
measured: one sweep found more stale sentences than three rounds of diff review
had. Decisions 6, 8, 16, and 18 also carry dated extensions from this sweep;
those record work that shipped as bound and are not counted above, because no
sentence in them was falsified.

_Extended (2026-07-26) — the second sweep, and the seventeenth._ The
crate-layout move (issue #180) opened this document again and ran the sweep the
rule above demands, against the repository at `06c3f7a`. Most live claims were
re-checked in the source and held: the four catalogue counts (`FORM` 13, `TYPL`
39, `RIDL` 31, `MANI` 13), the `RIDL-` free-code enumeration, decision 13's
seven-codes-in-source figure, decision 15's two `WorkspaceOutput` fields,
decision 20's three `E2 ledger` citations that still name no issue, and decision
21's account of what PR #175 shipped. The move itself falsified nothing here: it
changed the directories four crates sit in, which decision 9 records in a dated
note rather than by rewriting its own text.

The sweep's first report claimed **decisions 16 to 20's shipped bindings** all
held, and that claim was wrong about decision 19 — in the same commit whose own
findings list named a copy of Appendix A that decision 19's binding does not
cover. Review of that sweep caught the contradiction, and the seventeenth stale
sentence with it (`_Corrected_` at decision 19). This is the third time a
correction's own review, rather than the correction, is what found the miss, and
the second time the falsifying evidence was sitting in the correcting commit.
The lesson the mechanism keeps teaching: a sweep that reports "everything held"
has to be checked against what the same change reports elsewhere.

_Extended (2026-07-26) — the third sweep, and the first change to anticipate its
own stranding._ The epic E2 gardening pass opened this document to record that
decision 20's repointing had happened, and ran the sweep the rule above demands
against the repository at `4cffb74`. It found **no pre-existing stale
sentence**. Re-derived from the source rather than read off the previous sweep,
and holding: the four catalogue counts (`FORM` 13, `TYPL` 39, `RIDL` 31,
`MANI` 13) and decision 21's `RIDL-` free-code enumeration, both counted inside
the `diag_codes!` invocation; decision 13's seven-codes-in-source figure;
decision 6's three shipped service codes, enumerated from `diag.rs` and from
ridl §16.4 separately and agreeing; decision 15's two `WorkspaceOutput` fields;
decision 16's two diagnostic messages and the module comment now citing ridl
§2.1; decision 19's six-file Appendix A extent, re-enumerated by listing every
file in the workspace containing `FaultPageResult` and classifying each;
decision 21's account of what PR #175 shipped, RIDL-111 and RIDL-142 still
unminted included.

Three sentences **would** have been stranded, every one of them by this pass's
own changes, and all three are corrected in the same commit rather than left for
a later sweep: the plan's path in `## Status` and in `## References` (the plan
moved to `docs/archive/` at epic close), and decision 20's "still name no issue"
(the three `check.rs` citations now name #172). The count above therefore stands
at seventeen — these three were not stale when the sweep began, and this is the
first revision to find its own stranding before shipping it rather than after.

The cross-check this note demands — that a sweep reporting "everything held" be
read against what the same change reports elsewhere — was run, and two of the
gardening pass's findings touch sentences here without falsifying one. Decision
7's "both the Rust and the TypeScript backends compile ridl interactions" is
true of the backends and of the corpus, which generates both from one IR v2, and
is **not** true of the `ridlc` command line: `ridl-backend-ts` is a
dev-dependency of `ridlc`, and `ridlc build --emit` offers `rust`, `c-header`,
and `ir-json` only. The decision claims a second backend over one IR and the
ADR-0002 §8 mapping, both of which shipped, so the sentence stands as written;
the missing emit path is recorded on #172 rather than read into it here.
Decision 11's gate enumeration is a rule, and it holds as a rule — but
`cargo fmt --all --check` sits in no `just` recipe, so `just verify` does not
enforce one of the five things this decision names (issue #182), and "CI is
still stuck" is a claim the repository cannot confirm or refute either way.

_Which sentences are dated, and which are live._ Every sentence here is one of
two kinds, and **the split does not follow the paragraph headings**. A sentence
that **describes repository state** — what a file said, what a test checked,
what no document carried, what a message read — is dated to the revision that
wrote it. It is deliberately left alone when the work it justifies ships:
rewriting it would delete the evidence the decision rested on. A sentence that
**states a rule, a decision, or an obligation** is live, and a sweep must check
it against the repository.

Both kinds appear under the same headings. Only `_What the sources say._`
(decisions 16 to 21) is dated end to end. Every other heading is mixed. Decision
21's `_What the guard has to be, which the existing two are not._` opens with
the before-picture — the hand-maintained arrays and what their guard did not
check — and closes with the remedy, which is live and has shipped. Decision 16's
`_What it binds._` quotes two diagnostic messages as they read on 2026-07-25 —
both read differently now, correctly — while the obligation in the same
paragraph is live and has shipped. Decision 15's `_What changed and why._`,
`_What it preserves._` and `_One constraint the shape records._` are mixed the
same way. So classify by what a sentence asserts, not by the heading above it.
**Never correct a dated sentence to today's state**; record what changed in a
dated note beside it, the way decisions 1, 16 and 18 do.

_`## Context` is dated by construction, and is not swept._ That section records
the situation that prompted this ADR — including that the ridl reference and
general form disagreed on four points, which PR #173 has since reconciled
(decision 1). Correcting it forward would delete the reason the ADR exists, so
it is exempt in whole, the same way the evidence sentences above are.
`## Status` and `## Consequences` carry no such exemption and are swept, as this
note's own history records.

## Context

Epic E2 builds ridl — the interaction profile over the typl spine E1 shipped. It
adds the five interaction kinds (`signal`/`event`/`command`/`query`/`final`),
timing annotations, errors-as-data, the `require`/`ensure` contract subset,
`interface`/`service` declarations, IR v2, a second backend (TypeScript),
`ridl diff`, and the ridl LSP/lint surface.

Three tensions force decisions the plan cannot leave open:

1. **The ridl reference and general-form §6 disagree on four decided points.**
   The general form carries four supersessions the published ridl reference text
   has not yet absorbed — inline `T|E` returns (§6.1), generic min/max timing
   (§6.2), ordinal tooling (§6.3), and Stratum-3 wording (§6.4) — and the
   roadmap (E2.2/E2.3/E2.9/E2.10) explicitly cites the general-form versions.
   ADR-0007 decision 11 made the published reference outrank the general form
   for the E1 surface; E2 needs the inverse where the general form states a
   decided supersession, so the direction must be recorded, not assumed.
2. **Several surface points are under-specified or reopened** in the reference
   itself — the signal-init spelling, `persist`, the inline-`T|E` transport
   identity, the `final` keyword, and the service-code numbering.
3. **IR v2, the second backend, and `ridl diff` placement** need concrete
   choices the reference leaves to the toolchain.

CI is still stuck (ADR-0006 decision 8 / ADR-0007 decision 16); the local gate
remains the merge gate.

Decisions 16 to 21 are close-out amendments. The whole-epic review surfaced six
points where the shipped implementation and the specification text disagree, and
none of them was settled by the per-task work that produced decisions 1 to 14.
They arrive as amendments rather than as original decisions because the
close-out documentation sync cannot resolve them on its own: the sync can
restate a rule that has been decided, but it cannot decide which of two
disagreeing sources is the correct one.

## Decision

1. **Authority for the interaction surface: where general-form §6 states a
   decided supersession the ridl reference text has not absorbed, the general
   form is authoritative for E2.** This covers exactly four points — inline
   `T|E` returns (§6.1), generic `min`/`max` timing (§6.2), ordinal tooling
   (§6.3), and the Stratum-3 wording "infrastructure failure — detected,
   undeclared" (§6.4) — each cited by the roadmap. The ridl reference text is
   stale on these four and is corrected at epic close-out (a docs-sync item, the
   same way E1 closed out). Everywhere else the published ridl reference
   outranks the general form (ADR-0007 decision 11 still holds). **Extended
   (2026-07-26):** the close-out happened, so the staleness this decision routes
   around is gone. PR #173 absorbed all four supersessions into
   `docs/specification/ridl-language-reference.md`, whose header block now reads
   "Four supersessions from general form §6 are now absorbed rather than
   pending" and names them — §7, §10.1, Appendix A and Appendix C for inline
   `T | E`; §4.3, §5.2 and §9 for generic `min`/`max` timing; RIDL-407 for
   ordinal drift; §10.3 for the Stratum-3 wording. The authority rule itself is
   unchanged and still stands: where general form §6 states a supersession the
   reference has not absorbed, the general form governs for E2. It simply has no
   remaining case to govern.

2. **Signal init is the bare `= value` form, before any timing; signals carry no
   attribute block.**
   `signal targetSpeed : Speed = SPEED_LIMIT_EU @[20ms..500ms]` — this matches
   the naming-ledger "unified to bare `= value`" entry and the Appendix C
   grammar. The general form's `[ init = X ]` assignment-attribute spelling is
   not adopted for signals. Because signals gain no attribute block, a
   signal-level `persist` attribute has no grammatical home in E2 (see decision
   3).

3. **`persist` is deferred out of E2.** Its wire-evolution category is undecided
   (general form open question 3) and signals carry no attribute block (decision
   2), so there is no coherent way to ship it in E2. It is recorded as deferred
   debt for a later epic that revisits the signal attribute surface.

4. **The synthesized transport identity of an inline `T|E` result union derives
   from the interface plus the interaction ordinal plus the ordered arm types.**
   The container union is structural (both arms remain named typl types); its
   transport identity is synthesized so it stays stable under compatible
   evolution. Fixing this rule in IR v2 closes general-form open question 7 and
   lets `ridl diff` compare inline-`T|E` interactions honestly.

5. **The `final` keyword spelling is frozen for E2.** The general form reopens
   `final` vs `fixed` vs `provisioned` (§6.5), but the published reference uses
   `final` decisively; E2 implements `final` (published reference outranks the
   general form for the surface, decision 1). The reconsideration stays open and
   is revisited only with evidence, not re-litigated inside E2.

6. **Service diagnostics keep the reference's codes RIDL-140 and RIDL-141.** The
   reference numbers them in the 1xx band while listing them under the §16.4
   evolution/profile table — a documented anomaly. E2 keeps 140/141 as written
   rather than renumbering, so the emitted codes match the reference a reader
   consults; the numbering anomaly is noted for a future reference cleanup.
   **Extended (2026-07-26):** the service codes are no longer only those two.
   RIDL-143 (a `service` publishing an `internal` interface, PR #168) was minted
   in the same band under the same §16.4 table, and RIDL-142 is reserved by
   decision 21 for a third. The anomaly this decision carried at two codes now
   stands at three shipped, four once RIDL-142 is minted.

7. **Both the Rust and the TypeScript backends compile ridl interactions; the
   second, neutrality-proving backend is TypeScript.** The E2 exit criteria
   require ridl interfaces to compile to Rust _and_ a second backend from one
   IR, so the E1 Rust backend is extended to the interaction kinds and services,
   and a new TypeScript backend is added. TypeScript was chosen over a protobuf
   backend because E3.3 (uxdl viewmodel bindings) builds on it; running the same
   IR v2 through two languages is what proves IR neutrality. Both follow the
   ADR-0002 §8 mapping principle: the ridl package maps to the target's native
   namespace (a Rust `mod`, a TypeScript module), and `internal` declarations
   map to the target's package-private mechanism (Rust `pub(crate)`, a
   non-exported TypeScript member).

8. **IR v2 lands at `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`, using the field
   numbers IR v1 pre-reserved for E2.** Interfaces and services take the
   `Package` fields left unassigned for package-level E2 additions; the five
   interaction kinds take the `Decl.kind` oneof members reserved for them;
   envelope-related additions take the reserved `Decl` fields; the stream type
   takes the reserved `FieldType.kind` members. IR v1 is retained until the v2
   lowering lands, mirroring the E1 v0→v1 transition. **Extended (2026-07-26):**
   that retention has ended. The v2 lowering landed in PR #144 and removed the
   v1 schema with it, so `crates/ridl-ir/proto/ridl/ir/` holds `v2` alone. The
   field reservations this decision consumed are still visible in v2's
   numbering; the v1 proto that pre-cut them is not.

9. **`ridl diff` lives in the `ridl` facade (over a `tools/` engine crate), not
   in `ridlc`.** The compiler stays a pure source→IR function — the minimal ISO
   26262 tool-qualification boundary — while diff compares two IR snapshots
   against a baseline (lockfile/git/registry), which is workflow. `ridl diff`
   carries plumbing-grade stability: stable flags, machine-readable output, and
   the defined exit codes **0 = compatible, 1 = breaking, 2 = error** (concept
   note §9.1), the same contract class as `ridlc --frozen`.

   **Extended (2026-07-26):** the engine crate moved and the decision did not.
   `tools/` was dissolved when every crate went to `crates/<crate-name>/`, with
   the directory named after the crate (issue #180). The diff engine is now
   `crates/ridl-diff`; `tools/fmt`, `backends/rust`, and `backends/typescript`
   became `crates/ridl-fmt`, `crates/ridl-backend-rust`, and
   `crates/ridl-backend-ts` in the same move. The boundary this decision draws
   is over crate **dependencies**, not over directories: `ridl-diff` is a
   dependency of the `ridl` facade and of nothing `ridlc` compiles, which is
   what keeps the compiler a pure source→IR function. Relocating it neither
   weakens nor strengthens that argument. Read "over a `tools/` engine crate"
   above as "over an engine crate outside `ridlc`". Dated sentences elsewhere in
   this ADR keep the old paths, because they are the evidence they were written
   as — decision 21's 2026-07-26 extension describes `tools/diff/src/lib.rs` as
   PR #175 left it, and rewriting it would claim PR #175 touched a path that did
   not exist when it merged.

10. **The expr-core specification (E2.12) is a document, and it lands before or
    with the E2.4 subset implementation.** E2.4 implements only the guaranteed
    subset — comparison, boolean connectives, arithmetic, enum access,
    tuple-field access, and duration comparison, over parameters, `result`,
    constants, enum values, and the interface's own signals — and rejects
    anything outside it with RIDL-306. The document fixes the full family
    contract-term grammar the subset is verified against, and the subset must be
    a genuine forward-compatible subset of the family expr core the E5.1
    function layer extends (never a throwaway).

11. **The local merge gate carries over unchanged.** CI is still stuck;
    `just verify`, `cargo test --workspace`, `cargo fmt --all --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`, and the
    `--no-default-features` wasm build remain the merge gate for every E2 PR
    (ADR-0006 decision 8 / ADR-0007 decision 16).

12. **Interaction timing in IR v2 is always a resolved concrete bound.** Every
    signal and event carries a resolved `min` and/or `max` duration (untimed
    does not exist beyond the parser), a mode discriminator (strict-periodic
    `@Xms` versus a range), and a default-applied-versus-explicit flag.
    Durations are exact-decimal microsecond values, consistent with the IR
    exactness rule, so RIDL-100 and the "changing the configured default is a
    contract change" diff rule are both expressible.

13. **The `RIDL-` diagnostic namespace is the E2 vocabulary, and E2 allocates
    these new codes** (stable, never reused). The namespace groups by hundreds —
    1xx timing/interaction/envelope, 2xx streams, 3xx contracts/errors, 4xx
    evolution/profile — following the reference §16 tables, and the error-index
    website (E4.2) inherits it as it inherits `TYPL-`/`FORM-`/`MANI-` from E1.
    The new codes are: FORM-106 (unknown attribute key), FORM-107 (attribute key
    not allowed on this declaration kind), and FORM-108 (duplicate attribute key
    in one block) — the family-general attribute-block validation of general
    form §4.3; RIDL-308 (a named result union in return position, steering to
    the canonical inline `T|E` of general form §6.1); RIDL-407 (an interaction
    ordinal changed against the baseline, the desk-time drift check of general
    form §6.3); and MANI-009 (an invalid `[defaults].timing` value). MANI-008 is
    already taken by E1 (a workspace member directory with no manifest), so the
    timing-default code is MANI-009. The `labels`/`deprecated` promotion to
    attributes (general form §4.7) is not among the roadmap-cited supersessions,
    so `deprecated`-without-reason keeps the E1 doc-tag code TYPL-405 and no
    attribute code is minted for it. **Extended (2026-07-25):** decision 21 adds
    RIDL-111 and RIDL-142 for two errors E2 shipped uncoded, so E2 allocates
    eight codes, not the six listed above. **Extended (2026-07-26):** PR #168
    minted **RIDL-143** for a `service` publishing an `internal` interface, so
    the figure is **nine** (issue #169). The nine are the six above plus
    RIDL-111, RIDL-142, and RIDL-143. Only RIDL-143 of those three is declared
    in `crates/ridl-core/src/diag.rs`; RIDL-111 and RIDL-142 stay reserved and
    unimplemented, so a reader counting this ledger's codes in the source finds
    seven. This ledger is what decision 18 treats as authoritative and what a
    later epic will read, so the figure is maintained here rather than
    reconstructed from decision 21.

14. **`ridl diff` classifies changes directionally, comparing the resolved IR,
    and reads a workspace-local baseline.** Direction is judged from the
    consumer's side. A change is breaking (exit 1) when it shifts or reuses a
    wire identity or narrows a consumer-visible guarantee — an ordinal
    insert-not-at-end, a reorder, a non-tombstoned removal, an interaction-kind
    change, a payload/parameter/return type change, a wire-width flip, a
    narrowed typl constraint, a timing floor lowered (`min` down) or staleness
    bound raised (`max` up) or a bound added or removed or the
    strict-versus-range mode flipped, an error-arm added/changed/removed or any
    other inline `T|E` transport-identity change (decision 4), a `require` added
    or its clause text changed, or an `ensure` removed or its clause text
    changed. The classifier compares canonical clause source text and does not
    attempt to prove that one clause implies another, so any `require`/`ensure`
    text change is treated as breaking. A change is compatible (exit 0) when it
    only relaxes or appends — a new interaction at the end, a `reserved`
    tombstone, a new declaration, interface, or service, a widened constraint, a
    timing floor raised (`min` up) or staleness bound lowered (`max` down) with
    the mode unchanged, a `require` removed, an `ensure` added, or
    `default_applied` flipped with identical resolved bounds. Because diff
    compares the resolved IR, editing `[defaults].timing` surfaces as the timing
    change it is on every defaulted interaction. The desk-time baseline is
    stored under `.ridl/baseline/` and written by `ridl baseline`; `ridl diff`
    and the baseline-aware desk check read it (a registry or git ref can supply
    it later). This realizes general form §6.3's "baseline-aware ridlc" in the
    facade, not in `ridlc`, consistent with decision 9. Contract clauses are
    carried in IR v2 as their canonical source text; a full expression tree
    arrives when the E5.1 function layer restructures the representation.

15. **Amendment (2026-07-25) — `ridlc`'s workspace output carries the checker's
    resolution and the `ridl.std` IR.** `WorkspaceOutput` gains two fields:
    `resolutions`, each checked package's `Resolution` in `checked` order, and
    `std_ir`, the lowered IR of the built-in `ridl.std` package.

    _What changed and why._ `ridl test` (E2.11a) resolved the names in a
    contract clause against one package's `decls`. On the layout every shipped
    corpus entry uses — a types member plus interface members that import from
    it — that resolves nothing: a parameter typed from a sibling member or from
    `ridl.std` was reported as having no generatable range, so essentially every
    precondition was skipped while the command exited 0, and a clause naming an
    imported constant raised an unbound reference and exited **1** on a
    workspace `ridl check` accepts. Both are one root cause — the runner had no
    access to what the checker resolved — and both are fixed by handing it that
    resolution. `Resolution` was computed inside `load_and_check` and discarded;
    `ridl.std` is deliberately absent from `Workspace::packages`, so its IR
    never reached `checked`.

    A caller **can** rebuild both from API that was already public — running
    `load_workspace`, then `resolve_package` and `check_package` per member, and
    `std_package` for the built-in. That is the alternative this decision
    rejects, and the reason is duplication rather than impossibility: it obliges
    a workflow crate to restate `load_and_check`'s load-and-resolve loop — the
    same members, in the same order, against the same `std` handle — where it
    would drift silently from the compiler's own loop the first time that loop
    changes. Returning what the pipeline already computed keeps the sequence in
    one place.

    What is genuinely not available by another route is the **alias mapping**. A
    lowered `Contract` carries only canonical source text, which spells the
    locally bound name, so an alias can be resolved only through the resolver's
    own symbol map. Reconstructing it by indexing declarations by their bare
    name across packages is not equivalent and is rejected: it mis-binds under
    an import alias, where the local name is the alias while the declaration
    keeps its own, and under a cross-package name collision. For a correctness
    tool an approximation that silently binds the wrong declaration is worse
    than a visible skip.

    _What it preserves._ Decision 9's tool-qualification boundary is unchanged.
    Both fields are outputs the pipeline already produced; nothing about
    source-to-IR lowering moves, and no new pass, input, or configuration is
    exposed. `compile_workspace` alone checks `ridl.std`, so `run_check` and
    `run_build` — the qualified command drivers — do no work they did not do
    before. `ridl test` stays in the `ridl` facade, consistent with decision 9:
    the compiler reports what it resolved, the facade decides what to do with
    it.

    _One constraint the shape records._ `resolutions` is positional — one entry
    per `checked` entry, filled in the same loop — and not keyed by package
    name, because a package name is not a key: two workspace members may declare
    the same `[package] name`, which the toolchain currently accepts with no
    diagnostic at all. A name-keyed view would hand one member the other's
    declarations, which is a silent wrong answer rather than a missing one.
    Cross-package references still resolve first-wins by name, because that is
    what `package_of` does and therefore what the checker did when it lowered
    them.

    _Corrected (2026-07-25)._ This decision's opening sentence described
    `resolutions` as keyed by package name, contradicting the paragraph above it
    and the merged source, where the field is a `Vec<Resolution>` in `checked`
    order. The phrase survived from the first revision, where the field
    genuinely was name-keyed — the shape that silently handed one member another
    member's declarations, and the regression this decision's own review caught
    — so it described the defect rather than the fix.

16. **Amendment (2026-07-25) — a duration literal carries five UCUM time atoms,
    and the specification is corrected to the lexer.**

    _What the sources say._ ridl §2.1 presents a three-row suffix table — `us`,
    `ms`, `s` — and typl §2.8 names the same three in its examples. The lexer
    accepts five: `is_time_atom` matches `us`, `ms`, `s`, `min`, and `h`, and
    `durations_lex_as_one_token` pins all five. `timing.rs` scales `min` by
    60_000_000 and `h` by 3_600_000_000, with its own tests. `@[1min..1h]`
    compiles clean today and lowers to a 60000000us rate floor and a
    3600000000us staleness bound.

    _What was decided, and what kind of decision it is._ The specification
    extends to five; the lexer is not narrowed to three. The material fact is
    that **no ADR, no roadmap item, and no specification authorised the two
    extra atoms** — the language surface was widened in the implementation
    without a recorded decision, and this amendment ratifies that widening
    retroactively. It is ratified rather than reverted because narrowing the
    lexer would reject programs that compile today, which is the worse of the
    two outcomes — not because the implementation outranks the reference. It
    does not: decision 1 and ADR-0007 decision 11 govern which text is
    normative, and neither is disturbed here. What the tests and the module
    comment naming `min` and `h` establish is that the widening was not a slip —
    enough to make ratifying it reasonable, not enough to make it authorised.

    _What it binds._ The close-out documentation sync writes all five rows into
    ridl §2.1 and extends typl §2.8's parenthetical, after which §2.1's table is
    the complete list rather than a partial one. Two diagnostic messages go with
    it, so this amendment is **not** documentation-only: the FORM-102 message
    for a fractional duration and the MANI-009 reason for a malformed
    `[defaults].timing` bound both read "must be a whole number of us/ms/s", and
    the FORM-102 one cites ridl §2.1 — so widening §2.1 without them leaves a
    diagnostic contradicting the section it names. The text already misleads:
    `@[10ms..90min]` compiles clean while the message tells a user only three
    units exist. The whole-number rule §2.1 also states is unaffected — the
    lexer merges a fractional number into one `Duration` token and the checker
    rejects it with FORM-102, so admitting `min` and `h` does not admit `1.5h`.
    The duration atoms are a **proper subset** of the curated UCUM atom table
    ADR-0007 decision 8 adopted, not the same vocabulary: `d` is a unit atom
    there and is not a duration atom here, so `@[1min..1d]` is a hard FORM-101.
    (`timing.rs`'s module comment cites "ridl §2.8" for the atom set, where
    ridl's table is §2.1 and typl's is §2.8; corrected with the rest.)

    **Extended (2026-07-26):** PR #173 shipped all of it, and the "not
    documentation-only" claim held — `crates/ridl-sem/src/timing.rs` changed in
    the same commit as the reference. Both messages now read "must be a whole
    number of us/ms/s/min/h", the module comment cites ridl §2.1, and ridl §2.1
    carries five rows against typl §2.8's five-suffix parenthetical. Re-verified
    against the built `ridlc` at `af7ef7c`: `@[1min..1h]` compiles clean,
    `@[1ms..1.5h]` is FORM-102 quoting the new message, and `@[1min..1d]` is
    FORM-101 at the `d`.

17. **Amendment (2026-07-25) — `@[X..X]` is a degenerate range that warns on
    both signals and events; ridl §9.2 drops the "invalid on events" clause and
    the "equivalent to `@Xms`" characterisation together.**

    _What the sources say._ ridl §9.2 states that `@[X..X]` "draws a warning
    (equivalent to `@Xms`, and invalid on events)". ridl §16.1 classifies
    RIDL-108 as a warning with no restriction by interaction kind, and
    `resolve_timing` emits it at warning severity from its kind-neutral range
    branch; a signal and an event both annotated `@[50ms..50ms]` each draw one
    warning and the compile exits 0. On one event carrying both forms the
    compiler errors on `@Xms` with RIDL-103 and warns on `@[X..X]` with RIDL-108
    — while RIDL-108's own message calls the two equivalent.

    The two clauses in §9.2 are not independent. "Invalid on events" is a
    corollary of the equivalence asserted two words earlier, combined with
    RIDL-103. Striking the events clause alone would leave a section that still
    calls the forms equivalent while the compiler rejects one and accepts the
    other.

    _The equivalence is the stale part, not the events clause._ "Equivalent to a
    strict period" is pre-supersession vocabulary from the four-cell
    debounce/refresh/throttle/TTL model general form §6.2 replaced — the
    supersession decision 1 already records. Under §6.2, `min` is a rate floor
    and `max` a staleness bound, one generic meaning, with the per-kind
    behaviour derived from the state-versus-occurrence semantics of the
    declaring keyword rather than from the annotation. Nothing in that reading
    turns `min == max` into a strict period: `@[X..X]` is a degenerate range — a
    rate floor equal to its staleness bound — warned because it is almost always
    a mistake.

    The toolchain agrees, and on this point the evidence is checkable rather
    than interpretive. A strict period carries a mode meaning its bounds do not:
    ridl §9 admits it on signals only, and §9.1 never defaults it because "an
    isochronous rate is always an explicit engineering decision (it drives rmdl
    base clocks)". The IR records that mode separately from the bounds
    (decision 12) — `@10ms` and `@[10ms..10ms]` lower to identical bounds and
    different modes, `TIMING_MODE_STRICT_PERIODIC` against `TIMING_MODE_RANGE` —
    and the `ridl diff` classifier calls a mode flip breaking in both directions
    whatever the bounds do, because rmdl clocks key on strict (decision 14). Two
    forms the toolchain classifies as a breaking change between cannot be
    equivalent. On an event the strict-period reading is not available at all:
    an event is occurrence-driven, so there is no publication schedule for a
    rate floor and a staleness bound to make periodic. That is why RIDL-103 does
    not fire on `@[X..X]`, and why the current behaviour is correct rather than
    inconsistent.

    _What was decided._ `@[X..X]` warns on both kinds. §9.2 drops the "invalid
    on events" clause **and** the "equivalent to `@Xms`" characterisation, and
    describes the construct in §6.2's terms. Escalating RIDL-108 to an error on
    events was the alternative and is the wrong one: it would reject programs
    that compile today in order to preserve the very characterisation that is
    stale.

    _What it binds._ The retired characterisation lives in three places, and all
    three are edited together. §9.2's prose is one. **ridl §16.1's RIDL-108 row
    is the second** — it reads "`@[X..X]` — equivalent to `@Xms`", so the table
    this decision cites above as evidence of kind-neutrality is itself carrying
    the wording being retired; leaving it would let a reader take the row as
    support and stop there. The third is RIDL-108's own message ("equivalent to
    the strict period `@50000us`"), rewritten to name a degenerate range, which
    regenerates the showcase diagnostics snapshot that pins it. Those are the
    only sites: every other RIDL-108 reference in the repository carries the
    code and its severity without the message text. The code's severity, span,
    and kind-neutrality do not change.

    _Corrected (2026-07-26)._ "Those are the only sites" was false when written,
    and so was the "three places" it depends on. The retired characterisation
    lived in **five** places. The two the enumeration missed are
    `crates/ridl-core/src/diag.rs`'s `RIDL_108` doc comment ("equivalent to the
    strict-periodic `@Xms`") and the worked example that provokes the code,
    `crates/ridlc/tests/corpus/ridl-diag-showcase/main/timing.ridl` ("Equal
    bounds — the same thing as `@250ms`"). The second states the claim in
    different words, which is why a search for "equivalent" did not reach it.
    Both were repaired by PR #173 in the same commit as §9.2 and §16.1, so the
    wording is gone from the repository; what stayed wrong until this sweep was
    the count and the closure of the enumeration here. Editing the two
    regenerated no snapshot: the showcase entry carries errors, so its IR, Rust,
    and TypeScript snapshots are one-line placeholders, and the diagnostic
    snapshot pins the message rather than the source comment. The rest of this
    decision is unaffected — RIDL-108 is still a warning on both kinds, verified
    against the built `ridlc` at `af7ef7c`: `@[50ms..50ms]` on a signal and on
    an event each draw one RIDL-108 and the compile exits 0, and `@50ms` on an
    event is still RIDL-103.

18. **Amendment (2026-07-25) — the FORM and MANI diagnostic code tables are
    written in the family overview, and each language reference cites them.**

    _What the sources say._ Decision 13 allocated FORM-106, FORM-107, FORM-108,
    and MANI-009. All four are declared in `ridl-core`, listed in `FORM_CATALOG`
    and `MANI_CATALOG`, emitted by the checker, and provoked by the ridl
    diagnostic showcase. All four are the checker's: `ridl-core` cannot depend
    on `ridl-sem`, so the manifest layer records `[defaults].timing` as an
    unparsed string and the checker is what validates it and raises MANI-009. No
    document under `docs/specification/` carries a FORM or MANI code table, or
    names a FORM or MANI code at all. The two namespaces appear in five
    documents, none of them a specification: ADR-0007, this ADR, the E1 and E2
    epic plans, and `docs/technotes/walking-skeleton-architecture.md`, which
    describes `ridl-core` as the single source of truth for
    `TYPL-`/`FORM-`/`MANI-`.

    _What was decided._ Both tables are written in
    `docs/specification/ridl-family-overview.md`, and each language reference
    cites them rather than restating them. The FORM namespace is surface syntax
    — lexical and parse errors, plus the general form §4.3 attribute rules — and
    the MANI namespace is the manifest. Neither belongs to one profile, so a
    per-language table would be five copies of one list, drifting apart as codes
    are added. This is the house rule the overview already applies to doctrines:
    index once, cite from each reference.

    _What it binds._ The tables themselves are the close-out documentation
    sync's work, not this ADR's. The per-language `RIDL-` tables in ridl §16
    stay where they are — those are profile codes, and §16 is where a ridl
    reader looks for them. The technote named above is reconciled with the
    overview rather than left to drift — it describes where the namespaces live
    in the code, which stays true, but a reader sent there for the codes
    themselves now has a table to be sent to instead.

    **Extended (2026-07-26):** PR #173 shipped both tables, as
    `docs/specification/ridl-family-overview.md` §7 — §7.1 for `FORM-` and §7.2
    for `MANI-` — and repointed
    `docs/technotes/walking-skeleton-architecture.md` at them. The overview is
    therefore now a specification document naming FORM and MANI codes, which the
    evidence paragraph above says none was; that paragraph is the state on
    2026-07-25 and is left as written.

19. **Amendment (2026-07-25) — Appendix A's `union FaultPageResult` is deleted
    when the appendix adopts the inline `T | E` return.**

    _What the sources say._ Appendix A declares `union FaultPageResult` and
    writes `query getFaultPage(filter: DiagFilter): FaultPageResult`. That draws
    RIDL-308, which steers a named result union in return position to the inline
    spelling general form §6.1 made canonical — the first of the four
    supersessions decision 1 adopts, already listed as documentation-sync work.
    Once the return is written `FaultPage | DiagError`, nothing references the
    union.

    An unreferenced union draws no diagnostic at all. RIDL-308 is a
    return-position lint: it fires only when a query's return type names a
    result union, so adopting the inline spelling silences it and leaves the
    declaration behind with nothing to flag it. The toolchain has no
    unused-declaration lint — TYPL-007 covers unused imports only. A worked
    example is teaching material, and dead vocabulary that no pass will ever
    report is what a reader copies without noticing.

    _What was decided._ The union declaration is deleted in the same edit that
    adopts the inline return.

    _What it binds._
    `crates/ridlc/tests/corpus/veh-cluster/cluster/appendix-a.ridl` is the
    appendix text compiled — identical to the appendix's code block apart from a
    four-line provenance header — so the corpus copy tracks the edit and four
    snapshots regenerate with it. The diagnostics snapshot loses its RIDL-308
    warning; the IR snapshot loses the `FaultPageResult` declaration and
    rewrites the query's return from a named value to a fallible pair; the Rust
    and TypeScript snapshots lose the generated union type and change the
    query's signature. The entry's NOTES records the RIDL-308 warning as a true
    statement about the appendix, and is edited with it. RIDL-308 keeps a living
    example either way: the diagnostic showcase provokes it independently,
    inside a service's inline shape.

    _Corrected (2026-07-26)._ "`…/veh-cluster/cluster/appendix-a.ridl` is the
    appendix text compiled" was false when written. **Three** copies of Appendix
    A are compiled or parsed in this workspace, not one, and the binding above
    covers only the first. PR #173 edited that one and the reference itself;
    both now read `FaultPage | DiagError`. The other two still declare
    `union
    FaultPageResult` and return it:

    - `crates/ridl-syntax/test_data/parser/ok/appendix_a_full_example.ridl`,
      with its parser corpus snapshot. Parsing is all this fixture does, so no
      semantic pass reaches it and nothing flags the retired spelling.
    - `APPENDIX_A_PACKAGE` in `crates/ridl-sem/src/check.rs`, which is the
      larger miss because it propagates. Two tests pin the retired shape —
      `appendix_a_named_result_union_returns_as_a_named_value` asserts the
      return lowers to `Named("FaultPageResult")`, and
      `appendix_a_lowers_clean_and_its_ir_v2_json_is_the_golden` asserts the
      package's only diagnostic is the RIDL-308 this decision removes. Its
      comment still reads "Appendix A is kept verbatim rather than rewritten —
      the gf §7 erratum that restates it is a documentation task, not this one",
      which is exactly the position this decision reversed. Its golden,
      `ridl_sem__check__tests__appendix_a_ir.snap`, therefore still carries the
      union, and `crates/ridl-backend-rust/src/tests.rs` reads that golden as
      its Appendix A input — so the retired spelling reaches
      `ridl_backend_rust__tests__appendix_a_rust_snapshot.snap` and
      `…_c_header_snapshot.snap` as generated code.

    What was decided does not change: Appendix A's named result union is
    deleted. What changes is the extent — **six** further files, four of them
    snapshots: the parser corpus snapshot, the checker's IR golden, and the two
    backend snapshots generated from that golden. Closing it is separate work,
    recorded against the `debt(E2)` issue #172 rather than done here, because
    regenerating a checker golden and two backend goldens is not a rename's to
    make. Nothing about RIDL-308's living example is affected; the diagnostic
    showcase still provokes it.

20. **Amendment (2026-07-25) — ridl §16.1's RIDL-110 row is narrowed to what the
    checker validates, and the difference is recorded as a known gap.**

    _What the sources say._ §16.1 reads "signal `= value` init override violates
    the payload type's constraints", classified as an error. The checker
    validates three things, and only when the payload names a scalar `type`
    declaration: a numeric literal, or a constant reference resolving to a
    numeric value, outside the type's declared range; a string literal shorter
    or longer than the declared length bound; and a string literal that does not
    match the type's `match` pattern.

    Three cases the row's wording covers are accepted in silence. A literal of
    the wrong kind — `= true` on an integer-backed payload — is stringified and
    lowered. A value off the declared `step` grid — `= 15.0` on
    `float [0.0..100.0 step 10.0]` — is never compared against the quantization.
    An override on a `struct`, `enum`, or `union` payload has no scalar bounds
    to violate, so `= 5` on a struct and `= 42` on an enum with no such member
    both compile clean. The checker already records this at the emission site,
    where the leniency is named as recorded debt and the §16.1 wording is called
    out as reading broader than the check.

    _What was decided._ The row is narrowed to the three checks that exist, and
    the three gaps are written down as a known gap rather than closed silently
    in either direction. Widening the checker is a separate change with its own
    justification; this decision neither pre-commits to it nor removes the
    reason to make it.

    _What it binds._ A documentation-sync edit to §16.1, and a gap entry with a
    named home: the consolidated **`debt(E2)` issue** opened at close-out on the
    E1 pattern — ADR-0007 decision 10's "the E1 debt issue", which exists as
    #135. No `debt(E2)` issue exists yet, and this gap must not be recorded only
    in prose: `check.rs` already cites an "E2 ledger" in three places (M1, M2,
    M3, the second of them this row's own emission site) and that pointer
    resolves to nothing. The same work that opens the issue repoints those three
    citations at it. The leniency is E1's rather than new in E2 — a struct
    field's declared init is treated the same way — so a later widening is one
    change across both, not a ridl-only fix.

    **Extended (2026-07-26):** the `debt(E2)` issue now exists — **#172**,
    opened by the close-out documentation sync (#173), with the RIDL-110 gap and
    its three silent cases written up and each one verified against the built
    `ridlc`. The §16.1 narrowing shipped in the same PR. What has **not**
    happened is the second half of the sentence above: `check.rs`'s three "E2
    ledger" citations (M1, M2 — this row's own emission site — and M3) still
    name no issue, so the pointer still resolves to nothing. #172 carries that
    repointing as one of its own items.

    **Extended (2026-07-26) — the repointing shipped.** The E2 gardening pass
    made the three citations read `issue #172, M1`, `issue #172, M2`, and
    `issue #172, M3`; the string `E2 ledger` no longer appears in any `.rs` file
    in the workspace, and #172 carries an M1/M2/M3 section for the three to land
    on. Each was re-verified against the built `ridlc` before being repointed,
    because a citation is worth repointing only if it still describes something
    true. M1: two `reserved oldOne` tombstones in one interface draw no
    diagnostic and the compile exits 0, while both occupy an ordinal — the
    signal after them lowers at ordinal 3. M2: all three silent cases still
    compile clean and lower a `declared_init` — `= true` on an integer-backed
    payload, `= 15.0` off a `step 10.0` grid, and `= 5` on a struct or `= 42` on
    an enum with no such member. M3: every key in `GF_ATTRIBUTE_KEYS` raises
    FORM-107 unconditionally, so no key is consumable and the flat list still
    suffices.

21. **Amendment (2026-07-25) — RIDL-142 and RIDL-111 are allocated for the two
    uncoded E2 errors, and `ridl-core` gains a `RIDL_CATALOG` and a
    `TYPL_CATALOG`, generated rather than hand-maintained.**

    _What the sources say._ Two E2 commits shipped hard errors carrying
    `DiagCode::NONE`, which render as a bare `error:` with no code.
    `check_service_name` rejects a service-name segment that is not lowercase
    (E2.13). `resolve_type_path` rejects a name used as a type when it refers to
    an interface — ridl §14.0's rule that an interface has no values and cannot
    sit in payload, field, or parameter position (E2.1b). Ten further uncoded
    errors predate E2 and are out of scope here — four elsewhere in `check.rs`
    and six across `ridlc`, `ridl-core`, and the resolver, every one of them E1
    by blame.

    Being uncoded puts both outside the coverage this epic otherwise guarantees.
    `every_ridl_profile_code_has_a_living_example` walks a list keyed by
    diagnostic code, so a diagnostic with no code cannot be listed and cannot be
    shown to have a living example; and the E4.2 error index, which gives every
    code an explanation and a fix, has nothing to key them on. Separately,
    `ridl-core`'s `diag` module defines `FORM_CATALOG` and `MANI_CATALOG`, each
    guarded by a completeness-and-ordering test, and defines no `RIDL_CATALOG`.

    _What was decided._ The service-name-segment error is **RIDL-142**, beside
    the service codes RIDL-140 and RIDL-141 that decision 6 kept as written. The
    interface-used-as-a-type error is **RIDL-111**, in the 1xx interaction and
    envelope band, beside the interface-body rule RIDL-107. Both numbers are
    free: the `RIDL-` codes allocated anywhere in the repository are 100 to 110,
    140, 141, 201, 202, 301 to 308, and 401 to 407. A `RIDL_CATALOG` is added,
    listing every RIDL code with its severity and summary — **and a
    `TYPL_CATALOG` with it.** typl is the other namespace with no catalog, and
    the larger one: 38 declared codes to RIDL's 30. The E4.2 error index draws
    on both, so adding only `RIDL_CATALOG` would close the smaller gap and leave
    the larger one looking closed. The fix-wave item covers both namespaces, not
    one. Allocating RIDL-142 also enlarges the cleanup decision 6 deferred: the
    1xx-band numbering of the service codes was a documented anomaly at two
    codes, and is now one at three. And both codes **extend decision 13's
    allocation ledger**, which enumerates what E2 mints as a closed list of six;
    with RIDL-111 and RIDL-142 the figure is eight. That list is what E4.2's
    error index and any later epic asking what E2 allocated will read, so it is
    annotated there rather than left to be reconstructed from here.

    **Extended (2026-07-26) — the free-code enumeration and the two counts.**
    RIDL-111 and RIDL-142 are still free and still unimplemented, but the
    enumeration above is no longer the whole allocated set: PR #168 minted
    **RIDL-143**, so the `RIDL-` codes allocated anywhere in the repository are
    100 to 110, 140, 141, 143, 201, 202, 301 to 308, and 401 to 407. RIDL-143
    sits in the 1xx band under the §16.4 table, so the cleanup decision 6
    deferred reached three codes through it rather than through RIDL-142;
    minting RIDL-142 will make four. Decision 13's ledger accordingly reads
    nine, not eight (issue #169). The two namespace counts moved with the same
    work: RIDL declares 31 codes rather than 30 (RIDL-143), and typl 39 rather
    than 38 — PR #175 found TYPL-303 emitted as a bare string with no constant
    at all and declared it. typl is still the larger of the two, which is what
    the comparison was made for.

    _What the guard has to be, which the existing two are not._ `FORM_CATALOG`
    and `MANI_CATALOG` are hand-maintained arrays, and
    `form_catalog_is_complete_and_ordered` and its MANI twin compare each array
    against a second hand-written list inside the test. That pair checks that
    the two lists agree on content and order; it does not check either list
    against the codes actually declared, because nothing connects them. A new
    `DiagCode` constant added to neither list compiles and turns no test red, so
    "complete" in the test name claims more than the test delivers — the same
    way a hand-maintained array shadowing an enum let a new `Category` variant
    reach `--explain` unguarded. The remedy available there — make the guard an
    exhaustive `match` and let the compiler enforce totality — is **not**
    available here: `DiagCode` is a newtype over `&'static str`, not an enum, so
    there is no variant set to match on. What works in this shape is to declare
    each code once, in a form that expands to both the constant and its catalog
    entry, so a code with no entry cannot be written at all. The two new
    catalogs are built that way, and FORM and MANI move onto it in the same
    change — leaving two namespaces on a guard that cannot fail while two others
    have one that can is worse than either state on its own.

    The same hole sits behind this decision's own argument for coding the two
    diagnostics. `RIDL_PROFILE_CODES` in `crates/ridlc/tests/corpus.rs` is a
    hand-maintained list of code **strings**, with no link to the declared
    constants at all, so a code missing from it is never checked for a living
    example and nothing turns red. An implementer could mint RIDL-111 and
    RIDL-142, add their catalog entries through the new mechanism, omit the
    showcase entries, and see a green suite — which is the guarantee this
    decision invokes as its reason for acting. The declare-once mechanism
    therefore covers the showcase list as well as the catalogs. If that proves
    impractical — the list carries a `Provoked` discriminator the catalogs have
    no equivalent of — the fix wave states so explicitly and records the
    showcase gap as separate work, rather than leaving this decision implying a
    guarantee that does not hold.

    Two subsystems reached the same idea independently, which is the argument
    for making it the house pattern — and the sibling case marks how far it has
    to go to be worth anything. The `tools/diff` fix wave hit this defect in
    `CATEGORIES` and established that an exhaustive `match`, the obvious remedy,
    does not close it on its own: rustc forces _an_ arm, not the right one, so a
    new variant that silently classifies compatible still compiles. What it
    shipped is a macro **inside the test**, expanding one list of names into
    both an exhaustive match and the array its assertions iterate, so a new
    variant stops the file compiling. Its own documentation records what that
    leaves open — the macro shadows `CATEGORIES` where it should produce it, and
    generating the array from a single declaration is named there as the
    structural close it did not reach, since an assertion comparing two lists
    can still be defeated by editing what feeds it. The catalogs take that step
    rather than stopping at the shadow: the declaration produces the catalog,
    not a second list to compare it against.

    _What it binds._ The implementation — minting the two codes, adding the two
    catalogs and their guard, moving FORM and MANI onto the same mechanism,
    adding a showcase entry for each new code, and adding the two rows to ridl
    §16.1 and §16.4 — belongs to a fix-wave PR, not to this ADR. RIDL-111 and
    RIDL-142 are reserved from the moment this decision is recorded and are not
    reused if that work is resequenced.

    **Extended (2026-07-26) — what the fix wave shipped, and how it differs.**
    PR #175 took the catalogue half and left the minting half. The paragraph
    headed _What the guard has to be_ opens with the before-picture that
    motivated the change — left as written — and its remedy shipped as
    described; two other passages above are now wrong about the repository, and
    are named below.

    - **The catalogues.** `diag_codes!` in `crates/ridl-core/src/diag.rs`
      declares each code once and expands to the constant and its catalogue row,
      and all **four** namespaces are on it — `FORM_CATALOG`, `MANI_CATALOG`,
      `TYPL_CATALOG`, `RIDL_CATALOG` — with `ALL_CATALOGS` generated from the
      same expansion. The paired hand-written lists inside the old FORM and MANI
      guards are gone. #175 also added a guard the decision did not ask for,
      `codes_written_as_string_literals_are_all_catalogued`, which scans the
      workspace's `.rs` files for code strings emitted without a `DiagCode`
      constant — the escape #172 recorded, and how TYPL-303 came to be declared.
    - **The showcase list is _not_ covered.** The escape clause above was taken,
      and stated explicitly rather than left implied: `RIDL_PROFILE_CODES` in
      `crates/ridlc/tests/corpus.rs` is still a list of strings with no link to
      the constants, the gap is now recorded as **issue #172**, and #175 proved
      it by minting `RIDL-150` in `RIDL_CATALOG` with no showcase entry and
      watching the suite stay green. So the sentence "the declare-once mechanism
      therefore covers the showcase list as well as the catalogs" describes an
      intent the fix wave did not reach, on the ground the clause after it
      names.
    - **`tools/diff` went the same way, not the shadowed way.** #175 moved it
      too: `declare_categories!` in `tools/diff/src/lib.rs` now produces
      `CATEGORIES` from the `Category` declaration, and the in-test macro that
      shadowed it — with the paragraph naming the shadow as the close it did not
      reach — was deleted. So `tools/diff` is no longer the sibling case that
      stopped short; it is the second subsystem on the house pattern. What #175
      added beyond it is the wildcard defence: `classify`, `explain`, and
      `category_word` each `#[deny]` `clippy::wildcard_enum_match_arm` and
      `clippy::match_wildcard_for_single_variants`, the second being the
      load-bearing one for a 21st variant. That is enforced by clippy and by
      nothing else, which is why `just build` now runs `just lint` as well as
      `cargo test`.

    Still outstanding, and still reserved: **RIDL-111 and RIDL-142 are not
    minted**. Neither appears in `diag.rs`, in ridl §16.1 or §16.4, or in the
    showcase, and both errors still carry `DiagCode::NONE`. The numbers are not
    reused.

## Consequences

- Positive: IR v2 reuses IR v1's pre-cut field reservations, so the interaction
  layer extends the schema without renumbering; the exact-decimal, alias-free IR
  invariants carry forward so `ridl diff` compares honestly; a second backend in
  a different language proves the IR is language-neutral before three more
  profiles depend on it; keeping diff out of `ridlc` preserves the compiler's
  tool-qualification boundary.
- Negative / accepted: the ridl reference text carries four stale sections from
  decision 1's supersessions, and three more that the close-out amendments
  retire — §2.1's suffix table (decision 16), §9.2's equivalence wording
  (decision 17), and §16.1's RIDL-108 and RIDL-110 rows (decisions 17 and 20) —
  until close-out reconciles them; `persist` is deferred (decision 3); the
  service-code numbering anomaly is carried rather than fixed (decision 6), and
  RIDL-142 grows it from two codes to three (decision 21); the `final` spelling
  ships under an open reconsideration (decision 5); the inline `T|E`
  transport-identity rule (decision 4) is an agent-taken derivation the
  registry/backends inherit; `WorkspaceOutput` is two fields wider (decision
  15), so a consumer now sees the resolver's output and not only the lowered IR;
  and five of the six close-out amendments bind changes under `crates/` —
  diagnostic messages, a corpus fixture and its snapshots, comment citations,
  and the new codes and catalogs — so only decision 18 is documentation-only and
  the close-out is not a documentation-only pass.
- **Extended (2026-07-26):** two clauses of the bullet above have been overtaken
  by the close-out PRs, and the rest still holds. **The reference staleness is
  closed.** PR #173 absorbed all four of decision 1's supersessions and
  corrected §2.1's duration table, §9.2's `@[X..X]` rule, §16.1's RIDL-108 and
  RIDL-110 rows, and Appendix C's grammar; the reference's own header block
  records the reconciliation and cites this ADR for it. **The service-code
  anomaly reached three codes by another route.** RIDL-142 is still reserved and
  unimplemented; what grew the anomaly from two to three is RIDL-143 (PR #168),
  in the 1xx band under the §16.4 table, and minting RIDL-142 will make four.
  Unchanged: `persist` is still deferred, `final` still ships under an open
  reconsideration, the inline `T|E` transport-identity rule is still an
  agent-taken derivation, `WorkspaceOutput` still carries `resolutions` and
  `std_ir`, and five of the six close-out amendments still bind changes under
  `crates/` with decision 18 the only documentation-only one.
- Review hook: each numbered decision is reversible at the cost of a small
  refactor before a later epic builds on it; the maintainer can veto any of them
  by reopening this ADR.

## References

- ADR-0002 — module system (the codegen mapping decision 7 follows, the lockfile
  baseline decision 9 uses).
- ADR-0004 — sequencing and stack (the frame this refines).
- ADR-0007 — E1 execution decisions (the pattern this follows; decision 11 the
  authority rule inverts for the four supersessions).
- docs/ROADMAP.md — epic E2 stories and exit criteria.
- docs/specification/ridl-family-overview.md — the home decision 18 gives the
  FORM and MANI code tables.
- docs/specification/ridl-language-reference.md — the language E2 builds.
- docs/specification/typl-language-reference.md — §2.8, the duration-atom list
  decision 16 extends.
- docs/wip/family-general-form.md — §4 (attributes) and §6 (the four
  supersessions decision 1 adopts).
- docs/wip/ridl-family-concept.md — §9.1 (the `ridl diff` exit-code contract,
  decision 9).
- docs/archive/2026-07-19-e2-ridl-interface-layer-plan.md — the execution plan
  citing these decisions, archived from `docs/wip/` at epic close.
