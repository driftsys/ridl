# Archive

Superseded documents and completed Superpowers working memory, kept for
provenance. Nothing here is normative — the current references live in
[`../specification/`](../specification/), [`../decisions/`](../decisions/), and
[`../technotes/`](../technotes/).

- **ridl-language-reference-v0.1.md** — the original combined RIDL reference.
  Superseded when its vocabulary half (§1–§11) became the typl reference and its
  interaction half became ridl v0.2.
- **uxdl-language-reference-v0.1.md** — the user-interaction layer as a separate
  family member. Retired by
  [ADR-0012](../decisions/ADR-0012-interaction-boundary-model.md): its semantics
  moved into ridl as the boundary model, and its readable spellings into
  [the rxdl reference](../specification/rxdl-language-reference.md). Kept for
  provenance — its coverage analysis, its operation-shape taxonomy, and its
  prior-art survey are the source material for both. Read it as prior work,
  never as current design.
- **2026-07-18-e0-walking-skeleton-plan.md** — the epic E0 (walking skeleton)
  implementation plan, archived verbatim from `docs/wip/` once the epic landed.
  There was no separate spec artifact for this session: the roadmap's Epic 0
  section plus [ADR-0006](../decisions/ADR-0006-walking-skeleton-execution.md)
  served as the spec. The gardened records are
  [ADR-0006](../decisions/ADR-0006-walking-skeleton-execution.md) and
  [the walking-skeleton-architecture technote](../technotes/walking-skeleton-architecture.md).
- **2026-07-18-e1-typl-tooling-spine-plan.md** — the epic E1 (typl + tooling
  spine) implementation plan, archived verbatim from `docs/wip/` once the epic
  landed. As with E0, the roadmap's Epic 1 section plus
  [ADR-0007](../decisions/ADR-0007-e1-execution.md) served as the spec. The
  gardened records are [ADR-0007](../decisions/ADR-0007-e1-execution.md) and
  [the as-built architecture technote](../technotes/walking-skeleton-architecture.md).
- **2026-08-04-e9-1-to-e9-6-execution-plan.md** — the execution plan for roadmap
  stories E9.1 to E9.6, archived verbatim from `docs/wip/` once the block
  landed. Unlike the E0, E1 and E2 plans, this one gardened as it went: each
  story wrote its own durable records in its own pull request, so the closing
  pass archives the plan and syncs the drift rather than writing the records up
  afterwards. The gardened records are
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md),
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md), the
  roadmap's Epic 9 status paragraph, and the ridl reference sections each story
  amended. The four design notes it was written from stay in `docs/wip/`: three
  are ratified and kept as the reasoning trail, and the fourth
  (`2026-08-03-schema-projection-design.md`) covers E9.7 to E9.11, which this
  block did not run.

- **2026-08-05-projection-name-transform-design.md** and
  **2026-08-05-projection-name-transform-plan.md** — a design/plan pair written
  while executing roadmap story E9.7, once execution found that the
  schema-projection note's tie-breaker did not discriminate between the two
  `snake_case` implementations, its injectivity requirement was unsatisfiable by
  any case-folding transform, and the shipped Rust backend already emitted
  non-compiling output on colliding names. Archived as a pair once E9.7 landed,
  unlike the schema-projection note itself, which stays in `docs/wip/` as the
  reasoning trail for the E9.8–E9.11 stories it still covers. The gardened
  record is
  [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
  which ratifies the schema-projection note, carries these corrections, and
  cites this pair for the measurements and the full task-by-task execution
  trail.
- **2026-07-19-e2-ridl-interface-layer-plan.md** — the epic E2 (ridl, the
  interface layer) implementation plan, archived verbatim from `docs/wip/` once
  the epic landed. As with E0 and E1, the roadmap's Epic 2 section plus
  [ADR-0008](../decisions/ADR-0008-e2-execution.md) served as the spec — the two
  landed in one PR. The gardened records are
  [ADR-0008](../decisions/ADR-0008-e2-execution.md), the roadmap's Epic 2 status
  line, and
  [the as-built architecture technote](../technotes/walking-skeleton-architecture.md).
  Read it as a plan, not as a description: it is written in the future voice, it
  cites the pre-#181 crate paths (`backends/typescript`, `tools/diff`), and
  several of its statements about the repository were overtaken by its own
  execution.
