# ADR-0008: Epic E2 Execution Decisions (ridl — the Interface Layer)

## Status

Accepted (agent-taken, maintainer-reviewable); the last numbered decision is a
later amendment, dated in its own text. Each numbered decision below was taken
to unblock the epic E2 execution plan
(`docs/wip/2026-07-19-e2-ridl-interface-layer-plan.md`) and is reversible at the
cost of a small refactor before a later epic builds on it. This ADR follows the
pattern of ADR-0006 (E0) and ADR-0007 (E1).

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

## Decision

1. **Authority for the interaction surface: where general-form §6 states a
   decided supersession the ridl reference text has not absorbed, the general
   form is authoritative for E2.** This covers exactly four points — inline
   `T|E` returns (§6.1), generic `min`/`max` timing (§6.2), ordinal tooling
   (§6.3), and the Stratum-3 wording "infrastructure failure — detected,
   undeclared" (§6.4) — each cited by the roadmap. The ridl reference text is
   stale on these four and is corrected at epic close-out (a docs-sync item, the
   same way E1 closed out). Everywhere else the published ridl reference
   outranks the general form (ADR-0007 decision 11 still holds).

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
   lowering lands, mirroring the E1 v0→v1 transition.

9. **`ridl diff` lives in the `ridl` facade (over a `tools/` engine crate), not
   in `ridlc`.** The compiler stays a pure source→IR function — the minimal ISO
   26262 tool-qualification boundary — while diff compares two IR snapshots
   against a baseline (lockfile/git/registry), which is workflow. `ridl diff`
   carries plumbing-grade stability: stable flags, machine-readable output, and
   the defined exit codes **0 = compatible, 1 = breaking, 2 = error** (concept
   note §9.1), the same contract class as `ridlc --frozen`.

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
    attribute code is minted for it.

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
    `resolutions`, each checked package's `Resolution` keyed by package name,
    and `std_ir`, the lowered IR of the built-in `ridl.std` package.

    _What changed and why._ `ridl test` (E2.11a) resolved the names in a
    contract clause against one package's `decls`. On the layout every shipped
    corpus entry uses — a types member plus interface members that import from
    it — that resolves nothing: a parameter typed from a sibling member or from
    `ridl.std` was reported as having no generatable range, so essentially every
    precondition was skipped while the command exited 0, and a clause naming an
    imported constant raised an unbound reference and exited **1** on a
    workspace `ridl check` accepts. Both are one root cause — the runner had no
    access to what the checker resolved — and both are fixed by handing it that
    resolution. Neither field can be reconstructed by the caller: `Resolution`
    was computed inside `load_and_check` and discarded, and `ridl.std` is
    deliberately absent from `Workspace::packages` so its IR never reached
    `checked`. Reconstructing the first by indexing declarations by their bare
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

## Consequences

- Positive: IR v2 reuses IR v1's pre-cut field reservations, so the interaction
  layer extends the schema without renumbering; the exact-decimal, alias-free IR
  invariants carry forward so `ridl diff` compares honestly; a second backend in
  a different language proves the IR is language-neutral before three more
  profiles depend on it; keeping diff out of `ridlc` preserves the compiler's
  tool-qualification boundary.
- Negative / accepted: the ridl reference text carries four stale sections until
  close-out reconciles it (decision 1); `persist` is deferred (decision 3); the
  service-code numbering anomaly is carried rather than fixed (decision 6); the
  `final` spelling ships under an open reconsideration (decision 5); the inline
  `T|E` transport-identity rule (decision 4) is an agent-taken derivation the
  registry/backends inherit; `WorkspaceOutput` is two fields wider (decision
  15), so a consumer now sees the resolver's output and not only the lowered IR.
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
- docs/specification/ridl-language-reference.md — the language E2 builds.
- docs/wip/family-general-form.md — §4 (attributes) and §6 (the four
  supersessions decision 1 adopts).
- docs/wip/ridl-family-concept.md — §9.1 (the `ridl diff` exit-code contract,
  decision 9).
- docs/wip/2026-07-19-e2-ridl-interface-layer-plan.md — the execution plan
  citing these decisions.
