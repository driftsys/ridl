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
(`2026-08-08-flatbuffers-projection-{design,plan}.md`) on 2026-08-09, and the
baseline-gate design/plan pair (`2026-09-13-baseline-gate-{design,plan}.md`) on
2026-09-13 — E9.10 and the Epic 11 stories that absorbed E9.11 (E11.2 and E11.4,
ADR-0018 decision 16), both now parked by the 2026-09-12 re-scope, read the E9.8
design note, from the archive.

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
  measurement. Rules `RA-01..36`, nine opens. First implementation is the first
  runtime, from the first consumer. **Not ratified** — but the ADR-0018
  amendment it implies (RA-X10) is written: that record's 2026-09-12 amendments
  retire the `ridl-rt` name collision and amend its decisions 3, 6, 15, 16
  and 17. Amended 2026-09-12 in four places (see its own header). For `ridl-rt`
  0.1, this note is superseded by
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) and
  [the `ridl-rt` design record](../design/ridl-rt.md); its numbered rules
  (`RA-nn`) that concern the codecs, the engine and the runtimes remain working
  memory here for the Epic 11 stories that take them.
- **2026-09-08-topology-vocabulary.md** — the nouns for the layer between a
  contract and the hardware: distribution, machine, process, component, service,
  interface, member, catalog, with one question each (V-01) and only addressed
  things carrying wire identity (V-02). Three trees rather than one hierarchy;
  offer a service, consume an interface; generation follows imports while wiring
  follows `requires`. A component is a sans-IO synchronous step machine with a
  sync or async pump — which corrects rsdl v0.1 §1.3, whose three properties
  belong to three different levels, and drops composites. Machine is verified
  against AUTOSAR Adaptive ("quasi a virtualized ECU-HW"); a `target` is not
  one, and `RTE` names the layer `ridl-rt` occupies. Restates the consumer's
  catalog record §2/§5 and adds: a catalog is declared in ridl because it owns
  an id space and a hash, one package one catalog, ids allocated-and-recorded
  per package, and a generation filter produces a view and never a catalog.
  Leaves rsdl with four declarations plus a lock. Full mapping table to
  Adaptive, Classic and OSGi, and the rejected names with their reasons. **Not
  ratified**; six opens, including the cross-catalog references question and
  what a breaking _deployment_ change is.
- **2026-09-12-release-scope-and-plugin-system-design.md** — the design note of
  the 2026-09-12 re-scoping session: the release scope (typl, ridl, rsdl
  finalized; rmdl deferred; Rust with three payload encodings and TypeScript
  through wasm; the runtime library, a frame specification and an optional
  WebSocket transport; the codegen plugin system with Kotlin as the first plugin
  after the release), the decisions with their alternatives, and the open items
  it carries. Supersedes the scope parts of
  [`2026-09-08-roadmap-simplification.md`](../archive/2026-09-08-roadmap-simplification.md),
  now archived; its S-15 and S-17 are applied by the roadmap rewrite, and it
  feeds the ADR amendments. **Not ratified.**
- **2026-09-12-rsdl-rewrite-decisions.md** — the decisions the rsdl rewrite
  starts from, and the language surface: five container declarations, `offers` a
  service and `requires` an interface, machines list their instances; a lone
  service stands for an implicit component; no process and no scheduling facts;
  named instances with a unit instance, redundancy derived; crossing kinds only,
  transport and fabric are configuration, posture reserved; attributes as the
  backend escape hatch; identity numbers kept out of the source at every level,
  a per-package lock file frozen by `ridl lock` at release; cross-catalog
  references allowed with the hash over the closure. Two identity studies
  summarised. Amends `2026-09-08-topology-vocabulary.md` §1, §3, §7 and five of
  its invariants; the note's §4 lists them. **Not ratified.**
- **2026-09-12-interface-id-study.md** and
  **2026-09-12-interface-id-study-2.md** — the two identity study reports that
  note's D-7 summarises, kept verbatim; the first ranked the carriers, the
  second simulated the merges and the diff. **Not ratified.**
- **2026-09-13-runtime-descriptors-design.md** — the two files an engine reads:
  a catalog descriptor per package (interfaces with frozen numbers, members with
  ordinal, kind, bounds and a max-size table per payload per encoding) and a
  self-contained system descriptor per deployment (producers, placements, links
  with crossing kinds, routing table, grants, the attribute map). Both
  FlatBuffers, lowered from the protobuf IR, which ADR-0014 keeps for the
  toolchain. **Not ratified.**
- **2026-09-13-catalog-descriptor-plan.md** — the twelve-task plan for the
  catalog half of that design: the `ridl-descriptor` crate with the schema and
  its planus-generated accessors, the verifier, provisional numbering, the
  catalog hash, the proto3 and FlatBuffers size bounds, the lowering,
  `ridlc build --emit catalog` and `ridl describe`. The system descriptor waits
  for the rsdl rewrite. **Not started.**
- **2026-09-13-step1-lanes-plan.md** and its four driver prompts,
  **2026-09-13-lane-a-ridl-rt-driver.md**, **2026-09-13-lane-b-rsdl-driver.md**,
  **2026-09-13-lane-l-lock-driver.md** and **2026-09-13-lane-c-typl-driver.md**
  — the coordination plan for the first part of roadmap step 1: `ridl-rt` 0.1.0,
  rsdl and its reference, the lock block of the rsdl note's D-7, and the typl
  debt, as four lanes that run in parallel with one driver session each. It
  decides the order, the gates, the model for each stage, and which lane may
  change a shared file when; each lane's own spec decides the design. The
  coordination issue is #328. Lane A (`ridl-rt` 0.1.0) landed; its own
  design/plan pair is archived — see
  [`../archive/README.md`](../archive/README.md). Lane B (rsdl) landed except
  for story E6.17, the catalog hash per region; its plan
  (`2026-09-15-rsdl-plan.md`) is archived too, and its durable records are
  [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) and
  [the rsdl implementation technote](../technotes/rsdl-implementation.md). The
  state of the other two lanes is on #328.
- **2026-09-16-lane-c-c4-driver.md** — the driver prompt for lane C's last
  stage, C4 (Epic 10). Its own file because C4 is the largest stage of the four
  lanes — ten live tasks, one pull request each — so it runs as four sessions
  rather than one, and because it uses a cheaper two-seat review across model
  families instead of the four-seat one the 2026-09-13 prompts describe — a cost
  decision with a risk the file names, not an evidence-driven one. It carries
  the model routing Sebastien set on 2026-09-16 and three rules C3 paid to
  learn. Archive it with the lanes plan.
- **2026-09-20-lane-k-driver.md** — the driver prompt for lane K, the
  FlatBuffers payload codec (story E11.7). Its stage K0 filed the two rows the
  codec work surfaced, E11.14 (the face and the codec reach `ridl build`) and
  E11.15 (`ridl-loopback`, split out of E11.9 so it does not wait on the frame
  specification). Stage K1 wrote the codec's design note, which takes twelve
  named decisions, and then its plan; the stages after K1 come from that plan.
  **Stages K1 to K8 have run and the lane is not closed**: every decision is
  built except D-11, the interaction face moving off its `ReprC` placeholder,
  which is blocked on driftsys/ridl#470. That is why this document is still here
  while its note and plan are archived — a stage, K9, is left to run. The codec
  as built is
  [`../design/flatbuffers-codec.md`](../design/flatbuffers-codec.md).
- **typl-value-objects-design.md** and **typl-value-objects-plan.md** — typl
  §1.1 promises validators across every backend and neither language backend
  emits one. Design plus a ten-task plan; amends ADR-0013 rather than minting a
  record. Roadmap: Epic 10.

ridl-boundary-model-review.md, superseded by ADR-0012, is archived too — see
[`../archive/README.md`](../archive/README.md).
