# Work in progress

Pre-ADR and preliminary documents — direction-setting drafts and working specs
that are not yet ratified as normative references. They graduate into
[`../specification/`](../specification/) (or an ADR under
[`../decisions/`](../decisions/)) as they settle.

Superpowers specs and plans live here while an epic runs and are archived
verbatim to [`../archive/`](../archive/) at epic close, or as a design/plan pair
once the story they cover lands. The epic E2 plan was archived by the E2
gardening pass on 2026-07-26, the E9.1 to E9.6 execution plan on 2026-08-04, the
E9.7 design/plan pair (`2026-08-05-projection-name-transform-{design,plan}.md`)
on 2026-08-07, the E9.8 pair (`2026-08-08-proto3-projection-{design,plan}.md`)
on 2026-08-08, and the E9.9 pair
(`2026-08-08-flatbuffers-projection-{design,plan}.md`) on 2026-08-09 — E9.10 and
the Epic 11 stories that absorbed E9.11 (E11.2 and E11.4, ADR-0018 decision 16)
still read the E9.8 design note, from the archive.

- **ridl-family-concept.md** — the concept note: motivation, cores, profiles,
  the platform/IR model, the naming ledger. Explicitly pre-ADR (feeds the
  not-yet-written ADR-0003). Parts of it are aspirational rather than as-built —
  §8.1's repository tree shows `spec/`, `backends/`, `runtimes/`, and `tools/`
  directories that the workspace does not have (every crate lives at
  `crates/<crate-name>/`, issue #180). Read §8.1 for the plumbing/porcelain
  model it argues for, which did ship, not for the layout it draws.
- **family-general-form.md** — the cross-profile surface rules (three
  declaration shapes, nine invariants, the attribute model). A pre-ADR working
  spec, and the one document here that other records depend on: ADR-0008
  decision 1 made it authoritative for four points of the E2 interaction
  surface, and three documents under `../specification/` cite it by section —
  the ridl reference (§6.1, §6.4), the family overview (§4.3, for the FORM-106
  to FORM-108 rows), and the expr-core specification (§4.2). **Unresolved:** a
  document cited that way is normative in effect while its folder says it is
  not. Promoting it is a ratification decision, not a gardening move, so the E2
  gardening pass left it here and recorded the tension on issue #172.
- **skill-ridl-authoring-outline.md** — outline for the agent-authoring skill
  (see ADR-0005). Forward-looking: the skill it outlines is roadmap story E8.2,
  which has not been built.
- Four 2026-08-03 design notes (ir-protobuf-encodings, rpc-response-bound,
  multi-interface-services, schema-projection) are archived, gardened into
  ADR-0014, ADR-0015 and ADR-0016 — see
  [`../archive/README.md`](../archive/README.md) for what each became.
- **2026-09-08-ridl-rt-design.md** — proposes `ridl-rt`, the runtime library
  every generated package links, and records a **scope decision**: ridl owns
  types, interactions and system, and descopes the engine. The store, the
  seqlock, the frame protocol, the subscription table, the platform traits and
  the sans-IO session leave for external packages, to be revisited when rmdl
  lands, because they are deployment decisions rather than language ones and
  execution is rmdl's subject. §0 lists which ADR-0018 decisions survive (3, 4,
  5, 6, 13, 15, 17, 18), which leave (2, 7, 8, 11, 12, 16) and which split (10),
  and Epic 11 collapses to this library, the two codecs and the deployment
  schema. The library itself: identity and the §3.1 envelope, `Provenance` +
  `Freshness` + `Sample`, `Payload<E: Encoding>` with `verify`/`decode`
  separated behind a private-constructor `Ref`, `Inline` for one-size payloads,
  per-kind interaction descriptors, and seven pulled ports — with `scan` and
  `generation` demoted to a `ScannableSignals` extension because they presume a
  walkable store rather than an interaction semantic. Structural verification
  stays flatc's and ridl emits only the typl checks; nothing on the path needs
  `unsafe`; `Access::CHECKED` defaults true and is skipped only on a
  measurement. Rules `RA-01..35`, ten opens. First implementation is the first
  runtime, from the first consumer. **Not ratified** — and the ADR-0018
  amendment it implies (RA-X10) is not written.
- **2026-09-08-roadmap-simplification.md** — the plan, not the design. 102
  stories and roughly 220 person-weeks remain against one part-time author, so
  the note argues the roadmap is serving a public language platform and one
  system's SSOT at once and should choose the second. Parks E3.4-E3.6, most of
  E4, all of E12, E13 and E7, and eleven of E8, keeping every identifier and
  giving each parked block the evidence that reopens it. **Revised three times
  on the day.** The language order is typl -> ridl -> rsdl -> rmdl (S-18);
  `ridl-engine` — store, seqlock, sans-IO core, platform traits — parks as a
  block (S-19), which
  [`2026-09-08-ridl-rt-design.md`](2026-09-08-ridl-rt-design.md) §0 reaches from
  the design side and grounds better, as a scope decision rather than a
  sequencing one. rmdl is finalised as a draft and not implemented (S-25); code
  generation narrows to Rust (S-26); Kotlin becomes a plugin, which returns
  **E4.5b, the plugin protocol, to the critical path** (S-27); TypeScript
  through wasm is ADR-0018 decision 6's existing answer (S-28); the finish test
  for typl and ridl becomes a stabilisation loop over a real contract rather
  than a date (S-29); the generated gateway is held off (S-30). **D-1 (S-33) is
  the open decision**: rsdl v1 (7 stories, 9.5 weeks) or an out-of-band
  deployment descriptor (2 stories, 3.0 weeks) for the service/interface/machine
  relations the emitter needs to derive interfacing rules — recommended the
  descriptor, under one constraint, _out-of-band authoring, in-band
  representation_, so `ridl diff` still classifies it. **S-34** hardens that
  from a recommendation into the only available path: rsdl §3.1 defines a
  component by the rmdl model it applies, rsdl §7 makes a target logical and
  never addressed where the requirement wants machine identity, and ADR-0018
  opens 1 and 7 already record the same doubt. **S-35** adopts the settled
  vocabulary instead — machine / service / interface, with `vm` rejected because
  the QNX host is a machine too — so the descriptor adds one noun to what ridl
  §14 already owns. Leaves 30 stories and 47 weeks with the consumer's runtime
  as the first and only runtime. Rules P-1..P-6, S-01..S-35, A-1..A-4, D-1.
  **Not ratified**; eleven opens, including whether the public-platform goal is
  deferred or abandoned (SR-X1), that one implementation now validates the whole
  trait layer (SR-X6), and what a breaking _deployment_ change is (SR-X10).
- **2026-09-08-topology-vocabulary.md** — the nouns for the layer between a
  contract and the hardware: distribution, machine, process, component, service,
  interface, member, catalog, with one question each (V-01) and only addressed
  things carrying wire identity (V-02). Three trees rather than one hierarchy;
  offer a service, consume an interface; generation follows imports while wiring
  follows `requires`. A component is a sans-IO synchronous step machine with a
  sync or async pump — which corrects rsdl §1.3, whose three properties belong
  to three different levels, and drops composites. Machine is verified against
  AUTOSAR Adaptive ("quasi a virtualized ECU-HW"); a `target` is not one, and
  `RTE` names the layer `ridl-rt` occupies. Restates `catalog-abi.md` §2/§5 and
  adds: a catalog is declared in ridl because it owns an id space and a hash,
  one package one catalog, ids allocated-and-recorded per package, and a
  generation filter produces a view and never a catalog. Leaves rsdl with four
  declarations plus a lock. Full mapping table to Adaptive, Classic and OSGi,
  and the rejected names with their reasons. **Not ratified**; six opens,
  including ABI-X2's cross-catalog references and what a breaking _deployment_
  change is.
- **2026-09-12-release-scope-and-plugin-system-design.md** — the design note of
  the 2026-09-12 re-scoping session: the release scope (typl, ridl, rsdl
  finalized; rmdl deferred; Rust with three payload encodings and TypeScript
  through wasm; the runtime library, a frame specification and an optional
  WebSocket transport; the codegen plugin system with Kotlin as the first plugin
  after the release), the decisions with their alternatives, and the open items
  it carries. Supersedes the scope parts of
  `2026-09-08-roadmap-simplification.md`; feeds the roadmap rewrite and the ADR
  amendments. **Not ratified.**
- **typl-value-objects-design.md** and **typl-value-objects-plan.md** — typl
  §1.1 promises validators across every backend and neither language backend
  emits one. Design plus a ten-task plan; amends ADR-0013 rather than minting a
  record. Roadmap: Epic 10.
- **2026-08-08-rust-generated-surface-design.md** — what the code generators
  emit and in what order. Three artifacts (domain types, wire schema, codec) on
  two axes (language, encoding), a two-flag CLI, and two phases: validated types
  plus their codec, then a client/server interaction face. Records four defects
  in the shipped Rust backend as evidence, and proposes an answer to ADR-0013
  open item 1. Depends on **typl-value-objects-design.md** for phase 1's
  validation half. Roadmap: E4.5, E9.8–E9.11, Epic 10. **Not ratified** — seven
  open questions, including whether codecs are code- or descriptor-driven.

ridl-boundary-model-review.md, superseded by ADR-0012, is archived too — see
[`../archive/README.md`](../archive/README.md).
