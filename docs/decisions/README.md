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

- **ADR-0013 — Codegen backend scope.** _Proposed._ Classifies a backend by what
  its target can faithfully represent: a **wire** backend (proto3, FlatBuffers,
  and the remaining typl Appendix D targets) emits the typl surface plus an
  interaction identity table and no interaction face, because it cannot express
  ridl §4.4 last-value, §4.5 provenance, or the §3.1 envelope; a **language**
  backend emits source. Also fixes which width layer each class reads, rules
  typl constants out of a wire schema, and makes typl §17.11 a precondition for
  FlatBuffers. Not epic-scoped: it binds every backend the workspace grows.

- **ADR-0014 — IR encodings.** Canonical protobuf JSON replaces the `serde`
  rendering on every surface — artifacts, baselines, and goldens — because the
  rendering that shipped is serde's view of the generated Rust structs and no
  conformant protobuf parser can read it. Adds prototext and binary emits over a
  build-time descriptor pool, fixes the canonical-form policy E4.5 cites (binary
  is canonical, JSON is derived and conformance-obliged, prototext is for
  inspection), and makes the `ridl.std` emit filter an exhaustive
  classification. Supersedes the rendering clause of ADR-0004 §4. Not
  epic-scoped: it binds the artifact every future backend consumes.

- **ADR-0015 — QoS absorption, RPC bounds, and the interface as the unit.** ridl
  expresses QoS as semantic obligation, never as a transport knob, so it
  _absorbs_ QoS rather than excluding it. `command` and `query` gain the range
  form of the §9 timing annotation — `min` is a call throttle, `max` a response
  bound — warned but never defaulted (RIDL-112), with a diff category of its own
  because `min`'s direction inverts on an RPC. States the coherence rule at the
  interface grain, makes a provided interface the generation unit, and lifts the
  one-interface restriction on `service` so that grain is real: a
  comma-separated shape list, per-interface ordinals keyed by name, flat
  addressing preserved, three diagnostics, and five diff categories. Not
  epic-scoped: it binds the language surface until superseded.

ADR-0001 and ADR-0003 are not present in this repository; ADR-0003 ("the family
decision") is noted as not-yet-written in the family overview, and ADR-0012
constrains it to four family members rather than five.
