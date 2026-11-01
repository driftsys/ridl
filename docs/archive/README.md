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
  Part B4 Task 9 is the record of the last piece of lane B to be built — the
  catalog hash per region, story E6.17 (driftsys/ridl#367), embedded by `ridlc`
  (see the rsdl implementation technote). Read the rest as a plan: its tasks are
  the sequence the implementation followed, not a description of the result.
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
  it corrected in the stage before — not as a description of the result. **Every
  decision is built**: the last, D-11, the face on the codec, was blocked on
  driftsys/ridl#470 until stage K9a took that as ADR-0019 decision 8, and landed
  in stage K9b on 2026-09-21. The face over the codec is
  [the interaction-face design record](../design/interaction-face.md).
- **2026-09-20-lane-k-driver.md** — the driver prompt lane K ran under, archived
  with the pair above when the lane closed on 2026-09-21. Its stage K0 filed the
  two rows the codec work surfaced, E11.14 (the face and the codec reach
  `ridl build`, driftsys/ridl#444) and E11.15 (`ridl-loopback`, split out of
  E11.9 so it does not wait on the frame specification, driftsys/ridl#445); K1
  wrote the note and the plan, and K2 to K9 are that plan's tasks. Read it for
  how the lane was run — the mechanics, the review rule, and the four facts
  about the tree it warns are easy to get wrong — not for the state of the code:
  its `THIS SESSION RUNS` line and its stage list record what was still to do
  when it was written. Four findings outlive it — driftsys/ridl#467,
  driftsys/ridl#469, driftsys/ridl#472 and driftsys/ridl#476 — and the closing
  comment on driftsys/ridl#328 carries each one's owner.
- **2026-09-26-enum-variant-pascal-case-design.md** and
  **2026-09-26-enum-variant-pascal-case-plan.md** — the design and the six-task
  plan for driftsys/ridl#506: the Rust backend's enum variants projected through
  the pinned `pascal_case` transform (`CHECK_ENGINE` becomes `CheckEngine`), and
  RIDL-149 extended to an enum's values, keyed on `pascal_case` alone. Archived
  as a pair once the code landed. The gardened records are
  [ADR-0016's 2026-09-26 amendment](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
  [ADR-0017 decision 5's 2026-09-26 amendment](../decisions/ADR-0017-proto3-projection-rules.md),
  and the ridl reference's §16.4 RIDL-149 row.
- **2026-09-25-async-face-design.md** — the design note for story E11.21, the
  async and blocking generated clients and `serve` over the `ridl-rt` substrate.
  Fifteen decisions, F-1 to F-15, disposed of by Sebastien on driftsys/ridl#530
  on 2026-09-26, decision by decision. The gardened records are
  [the interaction-face design record](../design/interaction-face.md) (rewritten
  from the note),
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 6
  and its 2026-09-26 amendment,
  [ADR-0021's 2026-09-26 amendment](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)
  (decisions 13 to 18), [the `ridl-rt` design record](../design/ridl-rt.md),
  [the `ridl-loopback` design record](../design/ridl-loopback.md),
  [the frame specification](../specification/frame-specification.md) (`busy`
  crosses, §5.3, §5.4, §5.6, §8 and §9.6), and the RA-20 restatement in
  `crates/ridl-backend-rust/src/face.rs`. Read the note for the reasoning behind
  a decision, cited by decision number (F-n) from the design record it became,
  not as a second description of the as-built face.
- **2026-09-25-async-face-plan.md** — the five-task implementation plan for
  stages F3 to F5 of lane F, over the fifteen decisions the design note above
  disposed. Read it as a plan: its tasks are the sequence the implementation
  followed, not a description of the result.
- **2026-09-25-lane-f-driver.md** — the driver prompt lane F ran under, stages
  F0 to F5, one lane of the same coordination as lane K and lane M
  (driftsys/ridl#328). Read its §6 for what each stage owes the Kotlin side —
  the parallel port in driftsys/ridlc-gen-kotlin, tracked as its own issues per
  stage. Read the rest for how the lane was run, not for the state of the code:
  its `THIS SESSION RUNS` line and its stage list record what was still to do
  when it was written.
- **2026-09-28-face-fixed-methods-traits-design.md** — the design note for
  driftsys/ridl#580 (split from driftsys/ridl#570): the generated face's fixed
  methods behind traits. Its §2 inventories every generated item for a collision
  between a fixed name and a member's, its §4 and appendix prove each Rust
  resolution fact the design depends on by a compile experiment at Rust 1.83 and
  at the pin, its §7 records the alternatives, and its §8 the nine choices
  Sebastien confirmed on 2026-09-28. The gardened records are
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 7,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 19,
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 5's 2026-09-28 amendment,
  [the interaction-face design record](../design/interaction-face.md),
  [the `ridl-rt` design record](../design/ridl-rt.md) ("The face traits") and
  [the `ridl-rt` by example technote](../technotes/ridl-rt-by-example.md). Its
  §5 line numbers and its §6 pin literal record what was true when it was
  written. Read it for the reasoning and the experiments, cited from the records
  it became, not as a second description of the as-built face.
- **2026-09-29-generated-name-collisions-design.md** — the design note for the
  Rust backend's generated-name collisions (driftsys/ridl#583, #587, #588, #423,
  #449, #453, #455, #416 and #424). Its §2 inventories every Rust namespace a
  ridl name reaches, its appendix holds the experiments (X-n) as `ridl build`
  workspaces with the rustc result of each, its §7 records the alternatives, and
  its §10 the thirteen decisions taken on Sebastien's behalf. The gardened
  record is
  [the name-collisions technote](../technotes/rust-backend-name-collisions.md),
  with the binding parts in the 2026-09-29 and 2026-09-30 amendments of
  [ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 20
  and [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision
  7's note. Its §5, §6 and §9 plan the pull requests and the 0.5.0 release, and
  its line numbers record what was true when it was written. Read it for the
  reasoning and the experiments, not as a second description of the as-built
  names.
- **2026-10-03-lint-foundation-design.md** — the design for spec 0 of the devex
  and agent tracks brief (driftsys/ridl#671): the lint registry, the `[lints]`
  table of `ridl.toml`, the levels, where they apply, and
  `ridl check --format sarif`. Its §2 holds the nine decisions, its §4.2 the
  lint name table, its §5 the resolution rules and its §10 the alternatives. The
  gardened record is
  [ADR-0024](../decisions/ADR-0024-lint-registry-and-levels.md), where design
  D-n is decision n. The user-facing description is
  [the lints page of the book](../book/lints.md), the output formats are in
  [the CLI reference](../book/cli-reference.md), and the amendments are in place
  in [ADR-0002](../decisions/ADR-0002-module-system.md) §4 and
  [ADR-0010](../decisions/ADR-0010-cli-conventions.md) decision 1. Most source
  comments now cite ADR-0024 and the other durable records; one still cites this
  design by section (§7.2), and one cites the plan below. Read this design for
  the reasoning, not as a second description of the as-built behaviour.
- **2026-10-03-lint-foundation-plan.md** — the task-by-task plan that
  implemented the design above, merged as driftsys/ridl#678. Archived verbatim.
  It has no gardened record of its own; the decisions taken while the plan was
  executed are the stage driver's additions to decisions 10 and 12 and decisions
  13 to 16 of [ADR-0024](../decisions/ADR-0024-lint-registry-and-levels.md).
- **2026-10-03-mcp-workspace-tools-design.md** — the design for piece 1a of the
  devex and agent tracks brief (driftsys/ridl#668, #677): eight read-only MCP
  tools over a workspace on disk, with overlays applied inside the loader. Its
  §2 holds the six brainstorming decisions, its §4.4 the approved amendment for
  rsdl component uses, its §7 the compatibility rules and its §10 the
  alternatives. Archived verbatim with its two plans; its links to the brief and
  to the skill outline point at `../wip/`. The gardened records are
  [ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md), where design
  D-1 to D-6 are decisions 1 to 6, the amended
  [ADR-0005](../decisions/ADR-0005-agent-enablement.md), and
  [the design record](../design/mcp-workspace-tools.md). Read the design for the
  reasoning and the plans' test lists, not as a second description of the
  as-built tools.
- **2026-10-03-mcp-workspace-tools-plan.md** and
  **2026-10-03-mcp-tools-followup-plan.md** — the nine-task plan that #668
  implemented, and the four-task follow-up that #677 implemented (rsdl uses in
  the review tools, one copy of the snapshot helpers, outputs that no test
  pinned, and small fixes). Archived verbatim. They have no gardened record of
  their own; the decisions taken while they were executed are in the pull
  request bodies and in decisions 7 and 8 of
  [ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md).
- **2026-10-04-design-lints-design.md** — the design for piece 1b of the devex
  and agent tracks brief (driftsys/ridl#694): five candidate workspace design
  checks, a corpus of public interface sets ported into RIDL under `evals/`, the
  precision rule that sets each check's default level and threshold from
  labelled corpus findings, `ridl_metrics`, and the eval task format. Its §2
  holds decisions D-1 to D-9, its §4 the five candidate rules, its §7 the
  calibration procedure and its §10 the alternatives. Archived verbatim apart
  from its link to the brief, which points at `../wip/`. The gardened records
  are [ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md),
  whose status maps the design's decisions to its own, and
  [the design lints design record](../design/design-lints.md). Its §4 still
  describes the two candidates that did not ship and the search-start
  thresholds; read it for the reasoning, not as a description of the as-built
  checks.
- **2026-10-04-design-lints-plan.md** — the fourteen-task plan that implemented
  the design above in #707, #712 and the calibration branch, with the 109
  rulings taken while it was executed. Archived verbatim. It has no gardened
  record of its own; the rulings that changed the design (the workspace-only
  graph, the language server call, the recall join, the summary and notes split,
  the count guard, the removal of the two dropped checks, and Sebastien's
  approvals of the task set, the adjudication and the summary) are in
  [ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md).
- **2026-09-13-runtime-descriptors-design.md** — the design for the two files an
  engine reads: a catalog descriptor per package and a system descriptor per
  deployment, decisions D-1 to D-10. Archived verbatim apart from relative links
  once the catalog descriptor landed (E16.1 to E16.6). The catalog half became
  [the catalog descriptor design record](../design/catalog-descriptor.md); the
  system descriptor waits for its own story.
- **2026-09-13-catalog-descriptor-plan.md** — the twelve-task plan for the
  catalog half of that design, re-baselined on 2026-10-03. Archived verbatim
  apart from relative links. What it built is described, as built, in
  [the catalog descriptor design record](../design/catalog-descriptor.md).
- **2026-10-03-lane-e16-catalog-descriptor-driver.md** — the driver for epic
  E16, which ran that plan (#377 to #382, then E6.17 #367). Archived verbatim
  apart from relative links and the corrections to its own §5 "D8" entry, made
  during the review of the gardening pull request. The gardened record is
  [the catalog descriptor design record](../design/catalog-descriptor.md); the
  decisions the stages took are in the driver's
  [§5 "Decisions taken under delegation"](2026-10-03-lane-e16-catalog-descriptor-driver.md#5-decisions-taken-under-delegation).
- **2026-10-05-layout-inputs-design.md** and
  **2026-10-05-layout-inputs-plan.md** — lane S, the layout inputs for backend
  plugins (Epic 17, driftsys/ridl#715 to #720): the deployment section of the
  codegen request, the depth rule, the rsdl keys `depth`, `slots` and `budget`,
  the size states and the binding overheads. The design's §8 holds the delegated
  decisions DD-1 to DD-59, which Sebastien reviews; its D-6 table row for
  RSDL-806 is superseded by DD-42. The plan's §1 was the stage driver for S2 to
  S4. The plan is archived verbatim apart from its spec path. The design is
  archived with these edits: the D-6 table re-flowed, with the note "Superseded
  by DD-42" added to the RSDL-806 row and a paragraph below the table; its
  lifecycle-rule link pointed at `docs/wip/README.md`; DD-53, DD-54 and DD-57
  corrected and a dated note added to DD-58 during the review of
  driftsys/ridl#736. The gardened records are
  [the codegen plugins design record](../design/codegen-plugins.md),
  [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) and
  [the rsdl implementation technote](../technotes/rsdl-implementation.md). One
  item stays open: the WebSocket binding row, driftsys/ridl#718.
- **2026-10-06-rpc-default-response-bound-design.md** and
  **2026-10-06-rpc-default-response-bound-plan.md** — the default response bound
  for an untimed `command` or `query` (driftsys/ridl#741): the manifest keys
  `command_timing` and `query_timing`, the built-in `[..1s]` and `[..3s]`, and
  RIDL-112 as the "took the default" warning. The plan is archived verbatim
  apart from its spec path. The design is archived verbatim. The decision lives
  in [ADR-0015](../decisions/ADR-0015-qos-absorption-and-rpc-bounds.md)
  (decisions 4, 6, 7 and 8, amended in place), whose alternatives table carries
  the design's rejected options. The reference, the book and
  [the interaction face design record](../design/interaction-face.md) describe
  the as-built behaviour. The ceiling and floor lints stay a follow-up
  (driftsys/ridl#748).
- **2026-10-06-trace-context-propagation-design.md** and
  **2026-10-06-trace-context-propagation-plan.md** — the optional trace context
  on calls and events in `ridl-rt` (driftsys/ridl#752): the `TraceContext` type,
  the last argument of `Caller::command`, `Caller::query` and
  `EventSink::raise`, the `trace` field of `Claim` and `RawOccurrence`, and the
  four delivery rules. Both are archived verbatim, by driftsys/ridl#758. The
  decision lives in
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 21,
  whose alternatives table carries the design's rejected options. The as-built
  behaviour is in [the `ridl-rt` design record](../design/ridl-rt.md).
- **2026-10-08-catalog-per-unit-design.md**,
  **2026-10-08-catalog-per-unit-plan.md**,
  **2026-10-08-catalog-per-unit-handoff.md** and **catalog-per-unit-handoff.md**
  — one catalog, one `interfaces.lock` and one interface-number space per unit,
  the unit being one `[package]` manifest and its directory tree
  (driftsys/ridl#777): the spec, the plan, the execution handoff, and the Kotlin
  heads-up with the release note and its migration steps. The design, the plan
  and the execution handoff are archived verbatim; `catalog-per-unit-handoff.md`
  was edited after archiving to add the release note's migration step. The
  as-built behaviour is in
  [the catalog descriptor design record](../design/catalog-descriptor.md), the
  ridl reference section 11, and
  [ADR-0002](../decisions/ADR-0002-module-system.md) sections 1 and 4.
- **2026-10-09-review-residuals-778-plan.md** — the follow-up to the
  one-catalog-per-unit work: tests that pin the baseline gate, the lock protocol
  and the descriptor lowering, the doc statements the branch review found false,
  and small code follow-ups (driftsys/ridl#778). The plan is archived verbatim;
  the issue is its spec. The behaviours it changes that a user can observe: a
  workspace member listed twice is loaded once (recorded in
  [ADR-0002](../decisions/ADR-0002-module-system.md) section 1); the RIDL-409
  rename hint appears for a unit whose root declares no package, and for a
  payload written bare in one package and qualified in another. It also changes
  a test harness: the corpus codegen snapshots are lowered over the scope
  `ridl build` uses. Two items of the issue were left open on the issue by
  decision. Its other decisions are implementation choices.
- **2026-10-09-catalog-compat-design.md**, **2026-10-09-catalog-compat-plan.md**
  and **2026-10-09-lane-h-driver.md** — the compatible catalogs list
  (driftsys/ridl#787) and the lane H driver. The design and the plan are
  archived verbatim; the driver is archived with the lane's rulings and closing
  line added. The as-built behaviour is in
  [the catalog descriptor design record](../design/catalog-descriptor.md),
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15, and the book
  chapter "Evolving an interface".
- **2026-10-10-on-request-encodings-design.md** and
  **2026-10-10-on-request-encodings-plan.md** — on-request payload encodings
  (driftsys/ridl#801), stage 1 of the WebSocket and payload formats steering
  note: the manifest's `[backend.<name>] encodings` list, a types-only build by
  default, the per-encoding size table `Sizes<E>` and the payload buffers, the
  lint RSDL-807, and the generic face. The plan is archived verbatim; the design
  is archived with its section 8 amended, for the RSDL-807 fact stated below.
  The plan covers sub-stage 1a only; the design's sub-stage 1b (the face reads
  its encoding from the port, section 7.1 and 7.2) was built separately, from
  the residual design and plan archived below
  ([`2026-10-10-on-request-encodings-1b-design.md`](2026-10-10-on-request-encodings-1b-design.md)
  and
  [`2026-10-11-on-request-encodings-1b-plan.md`](2026-10-11-on-request-encodings-1b-plan.md)).
  Two facts differ from the design as written: the `ridl-rt` break ships in
  0.9.0, not 0.8.0, because 0.8.0 was released before the work landed; and
  RSDL-807 compares only the backends that can carry a codec, the Rust emit and
  the plugins, not the schema emits (section 8 was amended to match). The
  rationale of the 1a decisions is in the design's sections 14 and 15 (DD-S1-1
  to DD-S1-22). The as-built behaviour is in the 2026-10-10 amendments of
  [ADR-0002](../decisions/ADR-0002-module-system.md) section 4,
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md),
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) and
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md), and in
  [the `ridl-rt` design record](../design/ridl-rt.md),
  [the interaction-face design record](../design/interaction-face.md) and
  [the codegen plugins design record](../design/codegen-plugins.md).
- **2026-10-10-docs-in-generated-code-design.md**,
  **2026-10-11-docs-in-generated-code-plan.md** and
  **2026-10-10-docs-in-generated-code-handoff.md** — documentation in the
  generated code (spec 2b): the doc fields of the codegen model, the Rust and
  TypeScript rendering, link resolution and the doc lints' reach into generated
  output. The three files are archived verbatim; the handoff is the brainstorm
  note the design was written from. The as-built behaviour and the rationale of
  decisions 1 to 7 are in
  [ADR-0028](../decisions/ADR-0028-docs-in-generated-code.md), the amendment of
  [ADR-0026](../decisions/ADR-0026-doc-comments.md), and
  [the codegen plugins design record](../design/codegen-plugins.md).
- **2026-10-10-on-request-encodings-1b-design.md** and
  **2026-10-11-on-request-encodings-1b-plan.md** — sub-stage 1b of the
  on-request encodings design (driftsys/ridl#801): the generated interaction
  face reads its payload encoding from its port type (`P: port::Encoded`,
  `P::Encoding`) with generated `where` clauses instead of naming
  `::ridl_rt::encoding::FlatBuffers`, and `ridl-loopback` is typed by the
  encoding its ports carry (`Loopback<E = FlatBuffers>`, the six role handles,
  `Handles<E>`, `with_encoding`). The design holds the sections of the archived
  full design that describe 1b, copied verbatim, with its own list of what
  changed since it was written; both files are archived verbatim. The plan's
  question Q-1 was answered (a): all six role handles carry `E`. The as-built
  behaviour is in
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 15, [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
  decisions 6 and 7,
  [the interaction-face design record](../design/interaction-face.md),
  [the `ridl-rt` design record](../design/ridl-rt.md) and
  [the `ridl-loopback` design record](../design/ridl-loopback.md). One record is
  not amended yet:
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) still says
  that no port implements `Encoded` and that the face reads it in later work.
  The plan's Task 7 is the standing instruction for that rewrite, which waits
  for a separate pull request.
- **2026-10-12-ws-binding-design.md**, **2026-10-12-ws-binding-plan.md** and
  **2026-10-11-lane-w-plan.md** — the WebSocket binding of the frame
  specification (rulings R-W-7 to R-W-19, the plan's R-P-1 to R-P-10) and the
  wire and runtime plan that preceded it (R-W-1 and R-W-2: the minimum Rust
  versions of `ridl-loopback` and `ridl-descriptor`; R-W-3: `ridl-ir` has no
  `rust-version` of its own; R-W-4: the generated crate does not build at the
  minimum with default features off, driftsys/ridl#874); archived verbatim. The
  as-built rules are in
  [the frame specification](../specification/frame-specification.md) §11.1 and
  [ADR-0029](../decisions/ADR-0029-stability-policy.md); the reasoning and the
  alternatives are in
  [the binding rationale technote](../technotes/websocket-binding-rationale.md).
  The archived files link to sibling working files that stay in
  [`../wip/`](../wip/).
- **2026-10-06-ws-and-payload-formats-steering.md** and
  **ws-and-payload-formats-handoff.md** — the steering note and its session
  prompt for the WebSocket transport and the payload formats, archived verbatim.
  The note's decisions A (on-request encodings), I (the encoding comes from the
  port type) and B (the WebSocket binding fixes no encoding) are built: see
  [ADR-0002](../decisions/ADR-0002-module-system.md),
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md),
  [the interaction face](../design/interaction-face.md),
  [the frame specification](../specification/frame-specification.md) §11.1 and
  [the binding rationale technote](../technotes/websocket-binding-rationale.md).
  Decision H (header and envelope sizes) is superseded by the binding design.
  Decision D (the `prost` oracle) and the `repr(C)` questions live on
  driftsys/ridl#264 and #317; the open measurement, JSON and catalog-reader
  questions are items 7 to 9 of ADR-0020's open list. Its `docs/wip/` paths are
  historic.
- **2026-10-12-ridl-vendor-design.md** and **2026-10-12-ridl-vendor-plan.md** —
  the design of remote units and `ridl vendor` (driftsys/ridl#48: the read path
  of part 1, decisions V-1 to V-9, the `vendor/` reservation and the command of
  part 2) and the plan that built it, archived verbatim. The delegated rulings
  made while building it are in the pull request bodies of #941 and #950. The
  as-built rules are in [ADR-0002](../decisions/ADR-0002-module-system.md) §7,
  [ADR-0010](../decisions/ADR-0010-cli-conventions.md),
  [ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md),
  [ADR-0029](../decisions/ADR-0029-stability-policy.md) and
  [the CLI reference](../book/cli-reference.md). The `manifest` and `cli` freeze
  rows landed in #954.
- **2026-10-11-release-0.10-design.md** and
  **2026-10-11-release-0.10-lanes-plan.md** — the release 0.10 design (decisions
  R-1 to R-9, the frozen surfaces, the freeze mechanism, the lanes) and the
  lanes plan (the rules for every lane, the gate table, the lane briefs),
  archived verbatim after 0.10.0 was released. The as-built policy, the check
  and the gate table are in
  [ADR-0029](../decisions/ADR-0029-stability-policy.md),
  [`../../freeze/README.md`](../../freeze/README.md) and
  [the stability chapter](../book/stability.md). The release departed from the
  design in four ways, recorded in the 2026-10-14 amendment of ADR-0029: the
  baselines stay at commits and not at `v0.10.0`, the `manifest` and `cli` rows
  came after the tag, the "Migrating from 0.8 to 0.10" chapter was not written,
  and the first crates.io publish failed at its verify job. The design's section
  4.2 describes the check as first imagined; ADR-0029 decision 3 is the
  authority. The design's lane L item 3 (ratify `family-general-form.md` into
  `docs/specification/`) did not happen; that file stays in [`../wip/`](../wip/)
  with its disposition in [`../wip/README.md`](../wip/README.md).

- **2026-10-15-release-0.11-plan.md** — the release 0.11 scope and plan: eight
  defect fixes (#880, #884, #925, #919, #863, #874, #864, #877) after the freeze
  baseline move (#962), with the alternatives considered. All landed in
  `v0.11.0`. The plan's order and the rejected scope options are the only
  content not recorded elsewhere.

- **2026-10-19-ridl-transport-ws-design.md** and
  **2026-10-19-ridl-transport-ws-plan.md** — the WebSocket transport crate
  `ridl-transport-ws` (driftsys/ridl#265): the `no_std` codec of the frame
  specification §11.1, the server side of RFC 6455 written in the crate, the
  sans-IO session core over a backend's observe-and-call ports, the blocking
  `serve`, and the done-when test across processes. Both are archived verbatim;
  the design carries the "(as built)" edits made during implementation. The
  as-built behaviour, with the decisions D-1 to D-25 and the alternatives
  considered, is in
  [the `ridl-transport-ws` design record](../design/ridl-transport-ws.md); the
  dependency decision is
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6 as amended 2026-10-19. The follow-ups are driftsys/ridl#984 to
  driftsys/ridl#989.

- **2026-10-20-ts-transport-ws-design.md** and
  **2026-10-20-ts-transport-ws-plan.md** — the TypeScript WebSocket package
  `@driftsys/ridl-transport-ws` at `packages/ridl-transport-ws/`
  (driftsys/ridl#982): the consuming side of the frame specification §11.1 for
  Deno and the browser, carrying payload bytes unread, the cargo example server
  of `ridl-transport-ws` its end-to-end tests run against, and the gates
  `ts-check`, `ts-e2e` and `ts-browser-e2e`. Both are archived verbatim; the
  as-built behaviour differs from them in places (the call table in
  `src/calls.ts`, the `unexpected-outcome` discard, an invalid call bound that
  rejects rather than throws, listeners guarded with `reportError`, the
  end-to-end tests attaching with FlatBuffers, and `./bootstrap` downloading the
  Deno release zip). The as-built behaviour, with the decisions D-1 to D-18 and
  the alternatives considered, is in
  [the `ridl-transport-ws-ts` design record](../design/ridl-transport-ws-ts.md);
  the dependency decision is
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6 as amended 2026-10-20. The follow-ups are driftsys/ridl#991 and
  driftsys/ridl#992.

- **2026-10-20-proto3-codec-design.md** and **2026-10-20-proto3-codec-plan.md**
  — the design note and the ten-task plan for the generated Rust proto3 payload
  codec, story E11.8 (driftsys/ridl#264): the `Payload<Proto3>` and
  `Sizes<Proto3>` emitted on request, the wire mapping the codec shares with the
  emitted `.proto`, `Declaration.proto3` in the codegen protocol, the canonical
  writer, the reader's acceptances and refusals, the absence rule in proto3
  terms, the `prost` oracle generated through `protox`, and the guard that keeps
  `prost` out of `ridl-rt` and of a generated crate. Both are archived verbatim.
  Its D-22 is the owner's final sizing ruling of 2026-10-20: a type whose size
  state is not bounded is refused, as the FlatBuffers codec refuses one, and the
  platform-wide payload ceiling is driftsys/ridl#996; the two earlier rulings of
  that day are recorded there as rejected alternatives. The durable record is
  [the proto3 codec design record](../design/proto3-codec.md), with the
  2026-10-20 amendments of
  [ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md),
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md),
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) and
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md). The
  follow-ups are driftsys/ridl#996 and driftsys/ridl#997.

- **2026-10-22-cabin-ws-server-plan.md** — the plan for the cabin WebSocket
  server and its Deno demo (step 2 of
  [the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md)).
  Archived with one edit, the link to the design note. The durable record is the
  example itself, `examples/cabin/server` and `examples/cabin/web`, run by
  `just cabin-ws-demo`;
  [the transport design record](../design/ridl-transport-ws.md) lists what
  remains (driftsys/ridl#989).

- **2026-10-22-cabin-browser-demo-plan.md** — the plan for the browser demo of
  the cabin temperature check (step 3 of
  [the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md)).
  Archived with one edit, the link to the design note. The durable record is
  `examples/cabin/web` (`check.ts`, `page.ts`, `browser.ts`), run by
  `just cabin-browser-demo`.

- **2026-10-22-cabin-wasm-verify-plan.md** — the plan for verifying the cabin
  temperature through the wasm codec in both demos (step 4 of
  [the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md)).
  Archived with one edit, the link to the design note. The durable record is
  `examples/cabin/web/check.ts` and its tests, the `_cabin-wasm` recipe, and the
  out-of-memory check in `packages/ridl-runtime`.

- **2026-10-22-ts-codec-table-design.md** and
  **2026-10-22-ts-codec-table-plan.md** — the design and the plan for the
  generated TypeScript codec table (step 5 of
  [the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md)):
  `<base>.codec.ts`, the `codec` manifest key and MANI-019, and the
  `ridl_type_id` export. Archived with two edits, the links to that design note.
  The durable record is
  [the codec table design record](../design/ts-codec-table.md).

- **2026-10-23-transcode-design.md** and **2026-10-23-transcode-plan.md** — the
  design and the plan for converting a payload between encodings:
  `ridl_rt::payload::transcode`, the `[backend.rust] transcode` manifest key and
  MANI-020, and the generated `transcode_<s>_to_<d>` functions. Archived with
  one edit, the plan's path to the design. The durable records are the
  2026-10-23 amendment of
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 13, [the `ridl-rt` design record](../design/ridl-rt.md) and
  [the generated code chapter](../book/generated-code.md).

- **2026-10-24-podbuf-design.md** — the design of the third payload encoding,
  `podbuf`: the intent and trust model, the first-fit layout law, the preamble,
  the descriptors and the stride, the value rules, `verify`, the evolution
  rules, strict and open enums, the alternatives and the measured evidence of a
  spike on a Cortex-R52 model. Archived verbatim, on 2026-10-27, once the codec
  landed. The durable records are
  [ADR-0031](../decisions/ADR-0031-podbuf-projection-rules.md), which restates
  the design as decisions corrected to the code and lists where it departs from
  the design, and [the podbuf codec design record](../design/podbuf-codec.md).
  Its plan stays in `docs/wip/` until Tasks 13 and 14 are built.

- **2026-10-30-size-rules-plan.md** — the plan for the code of the ADR-0030
  amendments of 2026-10-28 and 2026-10-30: `ridl_core::limits`, the `ridlc` size
  pass with RIDL-157, the structure-only `verify`, the `ridl-rt` default
  ceilings, the size refusals of the loopback and of both WebSocket transports,
  and the limits in the generated TypeScript codec. Archived verbatim, on
  2026-10-30, once the work landed; its Open questions section holds the rulings
  R-1 to R-26. The durable record is
  [ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md) and the design
  records its Status names.

- **2026-10-30-inline-lint-directives-plan.md** — the plan for
  driftsys/ridl#1124: the `// lint: allow <name> -- <reason>` comment directive,
  the `ridl_core::lint::directive` module, TYPL-411 and TYPL-412, and the shared
  carrier predicate in `ridl-syntax`. Archived verbatim, on 2026-10-31, once the
  work landed. The durable records are
  [ADR-0024](../decisions/ADR-0024-lint-registry-and-levels.md) and
  [the lints chapter](../book/lints.md).

- **2026-10-31-streams-stages-1-2-plan.md** — the plan for stages 1 and 2 of the
  streams work: the `streams` list in the catalog descriptor, and the stream
  port of `ridl-rt` with its loopback implementation and conformance tests.
  Archived verbatim, on 2026-11-01, once the work landed in driftsys/ridl#1154
  (squash `ed336b98`, 2026-10-31); the `runtime-crates` freeze row came back in
  driftsys/ridl#1159. Stages 3 to 6 continue from
  `docs/wip/2026-10-31-streams-design.md` and driftsys/ridl#1116, which stay in
  `docs/wip/`.

- **2026-10-30-ws-blocks-design.md** — the design note for the `block` frames of
  the WebSocket binding (frame version 2, binding version 2 under `ridl-ws-v2`):
  the block form, the reassembly rules of the consumer, the block size and the
  records to amend. Archived verbatim, on 2026-11-01, once the work landed in
  driftsys/ridl#1169 (ADR-0030 decision 15, driftsys/ridl#1129). The durable
  records are [the frame specification](../specification/frame-specification.md)
  §4, §5.5, §11.1 and §12,
  [ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md),
  [the WebSocket binding design](../design/ridl-transport-ws.md) and
  [the binding rationale technote](../technotes/websocket-binding-rationale.md).

- **2026-10-31-ws-blocks-plan.md** — the plan for the code of the WebSocket
  blocks: the codec of both clients, the provider, the vectors, the freeze dump
  and the end-to-end tests. Archived verbatim, on 2026-11-01, with the design
  note above.

- **2026-11-01-streams-stage-3-plan.md** — the plan for stage 3 of the streams
  work: the generated Rust face of a query whose reply is a stream of named
  elements, its caller side and its `serve`. Archived verbatim, on 2026-11-01,
  with the pull request that builds it (driftsys/ridl#1171). The durable record
  is [`../design/interaction-face.md`](../design/interaction-face.md). Stages 4
  to 6 continue from `docs/wip/2026-10-31-streams-design.md`.
