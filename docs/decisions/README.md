# Architecture Decision Records

- **ADR-0002 — Module system.** `package` / `import` / `as` / `internal`, the
  manifest, lockfile, and resolver.
- **ADR-0004 — Implementation sequencing and stack.** The build order and
  technology choices (companion to the roadmap). Amended 2026-09-12: §1's
  sequencing and the V1/V2 release definitions are superseded by the roadmap's
  two steps.
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
  FlatBuffers. Decision 7 adds field absence: `?` is declared once and realised
  per target — structurally where the target can, in-band from a value the range
  does not use where it cannot, never surfaced to consumers. Not epic-scoped: it
  binds every backend the workspace grows.

- **ADR-0014 — IR encodings.** Canonical protobuf JSON replaces the `serde`
  rendering on every surface — artifacts, baselines, and goldens — because the
  rendering that shipped is serde's view of the generated Rust structs and no
  conformant protobuf parser can read it. Adds prototext and binary emits, fixes
  the canonical-form policy E4.5 cites (binary is canonical, JSON is derived and
  conformance-obliged, prototext is for inspection), and makes the `ridl.std`
  emit filter an exhaustive classification. Supersedes the rendering clause of
  ADR-0004 §4. Not epic-scoped: it binds the artifact every future backend
  consumes. Three amendments came out of implementation: decision 12 retracts
  the infallible serialization return, decision 13 contains the prototext
  reader, and decision 14 moves JSON off `prost-reflect` onto `pbjson`-generated
  impls so the interchange artifact carries no recursion ceiling. The descriptor
  pool now serves prototext alone.

- **ADR-0015 — QoS absorption, RPC bounds, and the interface as the unit.** ridl
  expresses QoS as semantic obligation, never as a transport knob, so it
  _absorbs_ QoS rather than excluding it. `command` and `query` gain the range
  form of the §9 timing annotation — `min` is a call throttle, `max` a response
  bound — warned but never defaulted (RIDL-112), with a diff category of its own
  because `min`'s direction inverts on an RPC. States the coherence rule at the
  interface grain, makes a provided interface the generation unit, and lifts the
  one-interface restriction on `service` so that grain is real: a
  comma-separated shape list, per-interface ordinals keyed by name, flat
  addressing preserved, five diagnostics (RIDL-144 to RIDL-148), and five diff
  categories. Not epic-scoped: it binds the language surface until superseded.
  Two amendments came out of implementation review rather than design: ADR-0014
  decision 12 retracts that record's infallible serialization return, and
  decision 24 here requires an interface name to be unique across a service's
  shapes, live or retired, and makes a retargeted slot breaking. Amended in
  place on 2026-09-15 by the lock design (rsdl decision D-7): an interface's
  number comes from its package's `interfaces.lock`, the list is a set with no
  tombstone, the ordinal spaces are keyed on (package, interface number),
  RIDL-146 to RIDL-148 are retired, and the five slot categories are replaced by
  `ServiceInterfaceAdded` and `ServiceInterfaceRemoved` (decisions 12, 15, 17,
  18, 19, 20 and 24, each dated).

- **ADR-0016 — Schema projection and the pinned name transform.** The four
  properties every projection from IR identity to a target's namespace must
  satisfy, with injectivity restated as a checked property of the package
  (RIDL-149) because no case-folding transform can carry it; the pinned
  `snake_case` algorithm — `c_header.rs`'s, reversing the design note's choice
  on measured evidence — public in `ridl-ir`, with both backend copies deleted;
  the check in `ridl-sem`, over interaction members and parameters. Ratifies the
  schema-projection note and corrects three of its statements. Not epic-scoped:
  it binds every backend that projects.

- **ADR-0017 — The proto3 projection.** The rules the first wire backend needed
  that no earlier record supplied: how a foreign reference projects, where
  constraint information goes, and totality over names as well as over field
  numbers. Its decision 1 fixes `generate_with` as the API every later wire
  backend inherits. Scoped to proto3, but read decision 1 before writing another
  wire backend.

- **ADR-0018 — The runtime core, two encodings, and what the backends emit.**
  _Proposed._ Retracts the interaction layer the language backends shipped and
  restores it as a later phase, retires the extern-C face, fixes the payload
  encodings, moves the store and the dispatcher into Epic 11, and resolves the
  service-block conflict between ADR-0013 decision 2 and ADR-0016 decision 10.
  Not epic-scoped: it binds every backend and the runtime. Amended 2026-09-12 on
  decisions 3, 6, 15, 16 and 17, and on the name `ridl-rt`, which now belongs to
  the library rather than to the engine; decisions 3, 6 and 15 rest on ADR-0020,
  and decisions 16 and 17 on the re-scope's other decisions.

- **ADR-0019 — The FlatBuffers projection.** Seven rules the second wire backend
  needed: a union isolated in a wrapper table, a non-table union arm boxed,
  every typl struct a `table`, a map with no `(key)`, the target's own name
  scopes, `= null` on a field whose enum declares no zero member, and a name
  that reaches a word the validity oracle reserves emitted as it stands. All
  seven are FlatBuffers-scoped; none binds another backend.

- **ADR-0020 — The third payload encoding, the runtime layering, and the codegen
  plugin system.** _Proposed._ `repr(C)` joins proto3 and FlatBuffers as a
  payload encoding, and the encoding matrix settles the codec-in-wasm boundary
  as FlatBuffers; `ridl-rt` is one `no_std` crate with one cargo feature per
  encoding, with the runtimes and the transports outside it in both Rust and
  TypeScript; and a backend becomes an executable over
  `generate(CodegenRequest) -> CodegenResponse`, fed by a lowering step that
  derives the shared semantics once in the compiler. Not epic-scoped: it binds
  every backend this workspace or the ecosystem grows, and the runtime material
  in every language. Amends ADR-0018 decisions 3, 6 and 15, ADR-0013's target
  list, and ADR-0007 decision 13 — the last of those is the only amendment in
  the set that changes shipped code, because the Rust backend emits `#[repr(C)]`
  on fixed-layout structs today.

- **ADR-0021 — The `ridl-rt` 0.1 API: identity, the proof type, the port
  dispositions, and the release policy.** Fixes what the earlier records left
  open once the crate had to compile: `InterfaceNo` scoped by catalog and its
  width, `CatalogHash` as `[u8; 32]` SHA-256, a port checked against one catalog
  at construction, a failed `require`/`ensure` clause carrying no value, the
  driftsys/ridl#308 and #309 dispositions, the sealed `Encoding` and
  private-field `Ref` proof type, and what counts as a breaking `ridl-rt`
  change. Its 2026-09-20 amendment adds decisions 11 and 12: every port trait is
  implemented for `&mut P`, and the `&self`-only traits also for `&P`; and a
  runtime presents one handle per port role, with an aggregate handle for a face
  that needs several, while the crate itself adds no `Send` or `Sync` bound.
  Binds every consumer of `ridl-rt`: the Rust codegen, the two runtimes, and the
  ridl reference finalization pass (story E14.2).

- **ADR-0022 — The rsdl system in the IR.** Where the lowered rsdl system lives
  and what carries it: a `System` message in `system.proto`, its own artifact
  beside the package IR, named `<pkg.Name>.system.{json,txtpb,binpb}` and
  written by the three IR dump emits. Also which facts of rsdl §13 the IR states
  and which it does not (`tier` and `deprecated` are not lowered; producers are
  stated once for the closure; a `reserved` tombstone has no route; the catalog
  hash is not carried until story E6.17), that `ridl build` writes every
  artifact when the only errors are RSDL-7xx and still exits 1, and that
  `ridl diff` lists system changes under two headings with no verdict and only
  when both sides are source trees. Binds the IR every later consumer reads, the
  `ridl build` contract, and `ridl diff`.

- **ADR-0023 — The generated interaction face: entry point, clause translator,
  and call signatures.** Five decisions: four taken while implementing story
  E11.13, the in-process MVP of the face ADR-0018 decision 15 restores, and one
  added by the 2026-09-20 amendment. the Rust backend's contract-clause
  translator accepts one expression form and refuses every other with a
  `GenerateError`, never dropping a clause silently; the face is emitted from a
  companion entry point, `generate_face`, while `generate` stays exactly what it
  emitted before this story, following the precedent ADR-0017 decision 1 set; a
  `Provider` method takes its argument by reference, superseding the M1 design's
  by-value example, which cannot compile; and a consumer-side call returns a
  correlation on success and `SendError` on failure, closing a gap that design
  left open. The 2026-09-20 amendment makes that success half the call's own
  `Copy` correlation newtype, one per command and per query, so a query's
  correlation cannot be passed to an `ack`; and its decision 5 has a face hold
  its port by value, with no lifetime parameter. Binds every later story that
  extends the Rust backend's interaction face, until superseded: E5.1, Epic 10,
  and any later language backend that follows this precedent. The as-built face
  this record's decisions produced is
  [the interaction-face design record](../design/interaction-face.md).

ADR-0001 and ADR-0003 are not present in this repository; ADR-0003 ("the family
decision") is noted as not-yet-written in the family overview, and ADR-0012
constrains it to four family members rather than five.
