# Architecture Decision Records

- **ADR-0002 — Module system.** `package` / `import` / `as` / `internal`, the
  manifest, lockfile, and resolver.
- **ADR-0004 — Implementation sequencing and stack.** The build order and
  technology choices (companion to the roadmap).
- **ADR-0005 — Agent enablement.** Enabling AI agents to author and evolve RIDL.
- **ADR-0006 — Walking-skeleton execution.** E0-scoped execution decisions
  (workspace layout, protox, deferred crates.io reservation).
- **ADR-0007 — Epic E1 execution.** E1-scoped execution decisions (ungrammar
  tooling, diagnostic namespaces, corpus layout, `ridl-sem` split, IR exactness,
  scope cuts).
- **ADR-0008 — Epic E2 execution.** E2-scoped execution decisions (general-form
  authority for the interaction surface, IR v2 placement, the TypeScript second
  backend, `ridl diff` placement and its classifier rules, the `RIDL-`
  diagnostic allocations, and six close-out amendments).
- **ADR-0009 — Toolchain pin and gate parity.** The pinned Rust toolchain, the
  justfile as the single definition of every gate command, and what happens when
  a tool the gate needs is absent. Not epic-scoped: it binds every contributor.
- **ADR-0010 — CLI conventions.** The exit-code taxonomy (0/1/2) across
  `ridl`/`ridlc`, which clig.dev guidance applies and which does not (the
  `diff(1)`/`grep(1)` precedent for a verdict-carrying exit 1, not clig), and
  the fail-closed rule `ridl fmt` was brought into line with. Not epic-scoped:
  it binds the CLI contract for every future subcommand.

- **ADR-0011 — The provisioned-constant keyword.** ridl's `final` renamed to
  `fixed`, so both ridl and uxdl spell one concept one way; `final` removed from
  the reserved-word registry. Records the rejected candidates, the IR
  field-number invariant, and the diagnostic-code invariant. Not epic-scoped: it
  binds the language surface until superseded, and it supersedes ADR-0008
  decision 5.

- **ADR-0012 — The interaction boundary model.** Retires uxdl as a family member
  and gives ridl a boundary model instead: five interaction families (`dispatch`
  `presentation` `intent` `acquisition` `control`), the four correspondence
  obligations they carry, keyword spellings per family, and extensions that are
  spelling tables plus backends with no grammar, no IR nodes, and no semantics
  of their own. Promotes the attribute registry from an open question to a
  precondition and requires fail-closed diff classification. Not epic-scoped: it
  binds the language surface until superseded.

ADR-0001 and ADR-0003 are not present in this repository; ADR-0003 ("the family
decision") is noted as not-yet-written in the family overview, and ADR-0012
constrains it to four family members rather than five.
