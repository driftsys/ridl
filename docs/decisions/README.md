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

ADR-0001 and ADR-0003 are not present in this repository; ADR-0003 ("the family
decision") is noted as not-yet-written in the family overview.
