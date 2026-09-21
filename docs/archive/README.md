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
- **rsdl-language-reference-v0.1.md** — the architecture layer as first drafted:
  components as situated reactions, application-notation wiring,
  capability-class targets, `place`, transport and posture derivation, bundles,
  and declared redundancy. Superseded by the rewritten
  [rsdl reference](../specification/rsdl-language-reference.md), which replaces
  every one of those constructs. Kept for provenance — its prior-art survey, its
  coverage analysis against architecture and SDV frameworks, and its posture
  derivation are the material the rewrite reserves or reopens. Read it as prior
  work, never as current design.
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
  amended. The four design notes it was written from are archived below: three
  as the reasoning trail, and the fourth
  (`2026-08-03-schema-projection-design.md`) covers E9.7 to E9.11, which this
  block did not run.

- **2026-08-05-projection-name-transform-design.md** and
  **2026-08-05-projection-name-transform-plan.md** — a design/plan pair written
  while executing roadmap story E9.7, once execution found that the
  schema-projection note's tie-breaker did not discriminate between the two
  `snake_case` implementations, its injectivity requirement was unsatisfiable by
  any case-folding transform, and the shipped Rust backend already emitted
  non-compiling output on colliding names. Archived as a pair once E9.7 landed,
  unlike the schema-projection note itself, which is archived below as the
  reasoning trail for the E9.8–E9.11 stories it still covers. The gardened
  record is
  [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
  which ratifies the schema-projection note, carries these corrections, and
  cites this pair for the measurements and the full task-by-task execution
  trail.

- **2026-08-08-proto3-projection-design.md** and
  **2026-08-08-proto3-projection-plan.md** — the design/plan pair for roadmap
  story E9.8, the first wire backend (`ridl-backend-proto`): the two tiers
  ADR-0013 admits, the typl-surface mapping, the interaction identity table, and
  the RIDL-149 extension to struct fields that ADR-0016 decision 4 bound to the
  commit that starts projecting them. Archived as a pair once E9.8 landed;
  E9.9's FlatBuffers projection and E9.11's store and dispatcher still read the
  design note, now from here, beside the parent schema-projection note, also
  archived below. The gardened records are the roadmap's Epic 9 status
  paragraphs — which also carry the ADR-0013 decision 2 versus ADR-0016 decision
  10 conflict left for E9.11, and the payload-type imports that story inherits —
  the ridl reference's RIDL-149 row, and the CLI reference's `proto` emit. The
  story's own decisions — the emit ceiling, constraints as comments only, the
  inlining rule that reversed the well-known-type mapping, and name totality
  over proto3's three symbol scopes — are in no ADR: the first three are
  recorded in the design note, the fourth only in the branch's commit trail.
  Read the plan as a plan, not as a description: its task 6 maps
  `ridl.std.Duration` and `ridl.std.Timestamp` onto the protobuf well-known
  types, a mapping execution implemented and then reverted — the reversal and
  its reasoning are in the design note's blast-radius section.

- **2026-08-08-flatbuffers-projection-design.md** and
  **2026-08-08-flatbuffers-projection-plan.md** — the design/plan pair for
  roadmap story E9.9, the second wire backend (`ridl-backend-flatbuffers`): the
  same two-tier ceiling E9.8 held, projected onto a target whose constructs
  differ from proto3's, with every structural claim verified against `flatc`
  25.12.19 and `planus` 1.3.0 rather than reasoned from the records. Archived as
  a pair once E9.9 landed. The gardened record is
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md), which
  records the story's seven projection rules and cites the design note as the
  reasoning trail; the two amendments live in the records they amend — ADR-0013
  decision 6 carries the width-floor closure in place, and typl Appendix D
  records that this projection does not take its fixed-layout `struct` allowance
  — and the roadmap's E9.9 status paragraph carries the rest. Read the plan as a
  plan, not as a description: its Task 9 mints the record as ADR-0018, a number
  [the runtime-core record](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  took before this branch merged, so the record shipped as ADR-0019; and the
  plan predates two of the record's decisions — the union-arm box (decision 2,
  which reversed an instruction to refuse a non-table arm) and the planus
  reserved-word ruling (decision 7, from the branch's final review) — the first
  recorded as an amendment inside the design note's §3.1, the second noted in
  its §5.

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

- **2026-08-03-ir-protobuf-encodings-design.md** — design note on the IR's own
  protobuf encodings, gardened into
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md). Archived verbatim.
- **2026-08-03-multi-interface-services-design.md** — design note on composing
  multiple interfaces into one service, gardened into
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md). Archived
  verbatim.
- **2026-08-03-rpc-response-bound-design.md** — design note on the RPC response
  bound, gardened into
  [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md). Archived
  verbatim.
- **2026-08-03-schema-projection-design.md** — design note on schema projection
  and the pinned name transform, gardened into
  [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md).
  Archived verbatim.
- **2026-08-08-runtime-and-codegen-architecture.md** — the reasoning trail of
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md), which
  is still Proposed. Archived verbatim; ADR-0018 was amended from the 2026-09-12
  session's design note rather than from this one, on 2026-09-12 — its decisions
  3, 6, 15, 16 and 17, plus the `ridl-rt` name collision.
- **2026-08-09-interaction-layer-retraction-plan.md** — the plan executed by
  pull request #241. Archived verbatim.
- **ridl-boundary-model-review.md** — superseded by
  [ADR-0012](../decisions/ADR-0012-interaction-boundary-model.md); its own
  header lists the claims it got wrong. Archived verbatim.
- **2026-09-08-ridl-abi-design.md** — the first draft of the runtime-library
  design under the name `ridl-abi`; superseded the same day by
  [`docs/wip/2026-09-08-ridl-rt-design.md`](../wip/2026-09-08-ridl-rt-design.md),
  which renamed the crate to `ridl-rt` and extended the note. Archived with the
  consumer project's names replaced by generic wording.
- **2026-09-08-roadmap-simplification.md** — the plan behind the re-scope: 102
  stories and roughly 220 person-weeks against one part-time author, so the note
  argues the roadmap was serving a public language platform and one system's
  SSOT at once and should choose the second. Archived by the roadmap rewrite,
  which applies its S-15 (split the roadmap into a forward plan and a landed
  record) and its S-17 (narrow the platform ladder), and which supersedes its
  sequencing with the two steps the 2026-09-12 design note sets. Its S-16 — one
  issue per epic rather than one per story — was considered and not adopted; the
  tracker still mirrors one issue per story. Kept for the reasoning trail: the
  parked blocks, and the observation that reopens each, are carried into the
  roadmap's own parked table.
- **roadmap-landed-record.md** — the delivery narratives and story tables for
  Epics 0, 1, 2 and 9, extracted from `docs/ROADMAP.md` by the same rewrite so
  that the roadmap holds the forward plan alone. History, not a plan.
- **2026-09-13-baseline-gate-design.md** and
  **2026-09-13-baseline-gate-plan.md** — the disposition of three filed defects:
  `ridl baseline` refuses to publish an interaction removed with no `reserved`
  tombstone (new RIDL-408, exit 1, driftsys/ridl#315); an explicit `--baseline`
  path holding no snapshot is an input error instead of a silent pass
  (driftsys/ridl#235; #234 stays open, because no test yet builds a baseline
  directory whose subdirectories hold no snapshot); and a composite body reorder
  gets its own diff category, `Category::MemberReordered`, instead of the
  `constraint_changed` fallback (driftsys/ridl#314). Executed as two pull
  requests over disjoint crates, as the design's own D-6 called for: the gate
  and the explicit-baseline read in `crates/ridl` (driftsys/ridl#330, which
  closes #235 and covers #315 at the interaction level, its interface level
  staying open); the reorder category in `crates/ridl-diff` (driftsys/ridl#331,
  for #314, still open when this entry was written). Both pull requests wrote
  their own durable records as they executed, so gardening reconciled rather
  than authored most of them: the ridl reference's §11 (the
  publication-enforces-the-tombstone paragraph) and §16.4 (the RIDL-408 row),
  [ADR-0010](../decisions/ADR-0010-cli-conventions.md) decision 1 (the
  `ridl baseline` and `ridl check` exit-code cells), and the CLI reference's
  `ridl baseline` section and exit-code table (the publication gate, the
  empty-baseline refusal, and the `member_reordered` category). Gardening added
  what execution left open rather than decided: the ridl reference's §17 open
  questions 12-14 (whether the gate should also refuse a moved existing
  tombstone; a whole package removed or renamed bypassing it; and a whole
  interface or service removed, or a service whose form switches, bypassing it)
  and, on the `member-reordered-category` branch, the typl reference's §17 open
  questions 14-15 (value-aware enum/enumset comparison, and whether a mid-body
  insert should also carry `member_reordered`). The interface level of the same
  tombstone rule — a removed interface with no service-level `reserved`, and a
  provisional interface number — waits for the lock file the rsdl decisions
  note's D-7 describes; building it now would be building something the lock
  block deletes.
- **2026-09-13-ridl-mcp-v0-design.md** and **2026-09-13-ridl-mcp-v0-plan.md** —
  the design and plan for the JSON diagnostic contract,
  `ridl check --format
  json`, the `ridl-mcp` crate, and the `ridl lsp` /
  `ridl mcp` subcommands. The durable claims are in ADR-0005 (host coverage),
  ADR-0010 (exit codes), the CLI reference, and `crates/ridl-mcp/README.md`.
- **2026-09-13-vscode-extension-distribution-design.md** and
  **2026-09-13-vscode-extension-distribution-plan.md** — the design and plan for
  the extension's binary resolution, MCP registration, the `editor-v*` release
  train, and the install scripts. The durable claims are in ADR-0007's
  2026-09-13 amendment and `docs/technotes/toolchain-distribution.md`. The
  design's install-script interface (arguments, a registry `PATH` update) was
  replaced by environment variables in the plan; the technote records the
  interface as built.
- **2026-09-13-execution-driver-prompt.md** — the prompt that ran the two plans
  above as one branch. Its instruction to review before opening the pull request
  was wrong: `/review` needs an open pull request.
- **2026-09-13-ridl-rt-v0.1-design.md** and **2026-09-14-ridl-rt-v0.1-plan.md**
  — the design and plan for `ridl-rt` 0.1.0, lane A of the 2026-09-13 step-1
  coordination (driftsys/ridl#328). The durable records are
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) (the
  decisions R-1 to R-12 fixed once the crate had to compile) and
  [the `ridl-rt` design record](../design/ridl-rt.md) (the crate's six modules
  and full type and trait surface, as built). Read the plan as a plan, not as a
  description: its code blocks predate both decision A-1 (driftsys/ridl#348) and
  the API revision (driftsys/ridl#351), so several signatures differ from the
  shipped crate — `Command`/`Query::require` and `Query::ensure` return
  `Result<(), Violation>` rather than `Result<(), ()>` (predates A-1);
  `payload::Rule` has `Invariant` in place of `Step` (predates #351);
  `SignalWriter::invalidate` and `touch` return `()` rather than
  `Result<(), WriteError>` (predates #351); `Handler::settle` takes
  `Result<&[u8], Contract>` rather than `Result<&[u8], CallError>` (predates
  #351); and `WriteError` and `RaiseError` carry no `Contract` variant (predates
  #351). [The `ridl-rt` design record](../design/ridl-rt.md) and the crate's
  rustdoc describe the API as built.
- **2026-09-13-lock-design.md** and **2026-09-15-lock-plan.md** — the design and
  plan for the interface lock, lane L of the 2026-09-13 step-1 coordination
  (driftsys/ridl#328): the identity widths, `interfaces.lock`, provisional
  numbering, `ridl lock` with its `merge` driver, the publication refusals, and
  the retirement of the service slot model. The durable records are
  [ADR-0015's 2026-09-15 amendment](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md)
  (decisions 12, 15, 17, 18, 19, 20 and 24),
  [ADR-0010](../decisions/ADR-0010-cli-conventions.md) (the `ridl lock` rows),
  the ridl reference §11 and §14.5, the diagnostics RIDL-409 to RIDL-412, and
  [the CLI reference](../book/cli-reference.md). Read the design's §1 for the
  identity widths lane A consumed, and the plan as a plan: its twelve tasks are
  the sequence the implementation followed, not a description of the result.
- **2026-09-15-lane-m-driver.md**, **2026-09-16-interaction-face-v0-design.md**
  and **2026-09-17-interaction-face-v0-plan.md** — the driver, design and plan
  for lane M: the MVP of the generated interaction face, story E11.13, built
  deliberately out of ADR-0018 decision 15's sequence so the team has a face to
  write against ahead of the frame specification (E11.1) and the transport
  (E11.9). The durable records are
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) (the
  contract-clause translator, the `generate_face` entry-point split, and the
  `Provider`/`Client` call signatures settled during the lane's implementation
  stage) and [the interaction-face design record](../design/interaction-face.md)
  (the face's architecture as built, and every placeholder it carries with the
  story that replaces it). Read the design note's §11 and the plan's "Settled M2
  decisions" section as the reasoning trail behind ADR-0023, not as a second
  description of the as-built face: two of the plan's own settled decisions (the
  clause translator and the entry-point split) were found necessary only once
  implementation started, after the design was approved.
- **2026-09-15-rsdl-plan.md** — the parse, check and lowering plan for rsdl,
  lane B of the same coordination (driftsys/ridl#328): the rsdl profile of the
  grammar, the workspace-level checks, the lowering to the IR's `System`
  message, `ridl diff` at the system, and the book chapter. The durable records
  are [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) (the carrier,
  the build gate and the diff),
  [ADR-0014's 2026-09-18 amendment](../decisions/ADR-0014-ir-encodings.md) (the
  system artifact's names), the rsdl reference §13 and §14, and
  [the rsdl implementation technote](../technotes/rsdl-implementation.md). Its
  Part B4 Task 9 is the record of the one piece of lane B that is not built —
  the catalog hash per region, story E6.17, which waits for
  `ridl_descriptor::hash::catalog_hash` (driftsys/ridl#324). Read the rest as a
  plan: its tasks are the sequence the implementation followed, not a
  description of the result.
- **2026-09-20-flatbuffers-codec-design.md** and
  **2026-09-20-flatbuffers-codec-plan.md** — the design note and the seven-task
  plan for the FlatBuffers payload codec, story E11.7, run as lane K of the same
  coordination (driftsys/ridl#328). Twelve decisions: where the
  `Payload<FlatBuffers>` implementations are emitted, how `encode` builds a
  back-to-front format into a caller's slice, where the typl constraint check
  runs so `decode` stays infallible, who owns the `MAX_SIZE` computation Epic 16
  also needs, and how the generated face stops being bound to `ReprC`. The
  durable record is
  [the FlatBuffers codec design record](../design/flatbuffers-codec.md), with
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) for the
  projection the codec agrees with and
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 7
  and 8 for what `ridl-rt` owes it. Read the note for the reasoning behind a
  decision and for §4a to §4e, each stage's record of what it measured and what
  it corrected in the stage before — not as a description of the result. **One
  of its decisions is not built**: D-11, the face on the codec, blocked on
  driftsys/ridl#470, so [the lane driver](../wip/2026-09-20-lane-k-driver.md) is
  still live.
