# Work in progress

Pre-ADR and preliminary documents — direction-setting drafts and working specs
that are not yet ratified as normative references. They graduate into
[`../specification/`](../specification/) (or an ADR under
[`../decisions/`](../decisions/)) as they settle.

Superpowers specs and plans live here while an epic runs and are archived
verbatim to [`../archive/`](../archive/) at epic close, or as a design/plan pair
once the story they cover lands. **None is open.** The epic E2 plan was archived
by the E2 gardening pass on 2026-07-26, the E9.1 to E9.6 execution plan on
2026-08-04, the E9.7 design/plan pair
(`2026-08-05-projection-name-transform-{design,plan}.md`) on 2026-08-07, and the
E9.8 pair (`2026-08-08-proto3-projection-{design,plan}.md`) on 2026-08-08 — E9.9
to E9.11 still read the E9.8 design note, from the archive.

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
- **2026-08-03-ir-protobuf-encodings-design.md** — the emitted `.ir.json` is a
  serde rendering of Rust structs, not protobuf JSON, so no non-Rust protobuf
  runtime can parse it. Proposes canonical protobuf JSON plus prototext and
  binary emits, and **ADR-0014**, superseding ADR-0004 §4's rendering clause.
  Roadmap: E9.1–E9.3. **Ratified 2026-08-04** as
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md); this note stays as the
  reasoning trail, including the measurements the record summarises.
- **2026-08-03-rpc-response-bound-design.md** — the first pass at ridl §17.5
  open question 5. Finds that ridl _absorbs_ QoS rather than excluding it, and
  proposes RPC bounds with a response bound, the coherence rule, and
  **ADR-0015**. Roadmap: E9.4, E9.5, E9.12. Its ADR was renumbered from 0013,
  which was taken before it was written — see the note in §8. **Ratified
  2026-08-04** as
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md), together
  with the multi-interface note below; the record's Status says why the two
  became one record rather than two.
- **2026-08-03-multi-interface-services-design.md** — lifts the one-interface
  restriction on `ServiceDef`, with per-interface ordinals keyed by name, flat
  addressing preserved, three diagnostics, and five diff categories. The design
  pass §11.1 of the RPC note called for. Roadmap: E9.6. **Ratified 2026-08-04**
  as decisions 12 to 19 of
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md). That
  record's decision 20, retiring the `Service` message's `oneof` field numbers,
  is new there rather than ratified from this note — see its Status.
- **2026-08-03-schema-projection-design.md** — the identity chain from ridl to
  proto3 and FlatBuffers, the projection contract, and the generated store and
  dispatcher shapes. Roadmap: E9.7–E9.11. **Ratified 2026-08-05** as
  [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
  which corrects three of its statements — the transform choice, the injectivity
  requirement, and the inline-shape unification. This note stays as the
  reasoning trail for E9.9–E9.11, which have not landed yet. Its own corrections
  were written up as a dedicated design/plan pair for E9.7
  (`2026-08-05-projection-name-transform-{design,plan}.md`), archived once that
  story landed — see [`../archive/README.md`](../archive/README.md).
- **2026-08-08-flatbuffers-projection-design.md** and
  **2026-08-08-flatbuffers-projection-plan.md** — the second wire backend. Every
  structural claim is verified against `flatc` 25.12.19 and `planus` 1.3.0
  rather than reasoned from the records, and three of them amend a record: a
  union is isolated in a wrapper table rather than hand-rolled (the
  schema-projection note §4.4 is superseded), a struct is always a `table`
  because a FlatBuffers `struct` fabricates a value after a compatible append
  (typl Appendix D's `fixed_layout` allowance is withdrawn for this projection),
  and ADR-0013 decision 6's width-floor precondition is closed by decision, with
  the measured cost of the alternative. Roadmap: E9.9.
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
- **ridl-boundary-model-review.md** — spike record from the uxdl design review
  of 2026-08-03, on the finding that datum and referent come apart at every
  boundary with the non-software world. **Superseded by ADR-0012**, which was
  written from it and is authoritative wherever the two disagree. Kept for the
  reasoning trail — the arguments, the retractions, and the falsification tests.
  Its own header lists the three claims in it that are known to be wrong.
