# Lane LIM — per-interaction payload limits

Created 2026-10-24. The main session drives the lane. The design is ADR-0030
decisions 10 and 12 to 15, as amended by driftsys/ridl#1056
(driftsys/ridl#1045). The owner accepted the amendment and the breaking changes
of stage LIM3 on 2026-10-24. Rulings the lane takes are recorded under
[Rulings](#rulings).

## Scope

1. **driftsys/ridl#1057** — read the `[limits]` table of `ridl.toml`, resolve
   each interaction's limit, report the manifest and name errors, and report
   RIDL-151 for a payload above a declared limit.
2. **driftsys/ridl#1058** — record each declared limit in the IR, cover it by
   the catalog hash, add it to the descriptor's member table, and report every
   limit change as breaking in `ridl diff`.
3. **driftsys/ridl#1059** — `ridl-transport-ws`: the 16,292-byte default, a
   setter that may only lower it, a request above its member's limit answered
   `corrupt`, and the retained-cap clamp removed. Breaking.

## Stages

| Stage | Work                                                  | Model                                                   | Starts when                     |
| ----- | ----------------------------------------------------- | ------------------------------------------------------- | ------------------------------- |
| LIM1  | #1057, from the plan below                            | Sonnet stage agent; implementer Sonnet; reviews per SDD | now                             |
| LIM2  | #1058; its plan is written when LIM1 lands            | Opus for the plan; Sonnet stage agent                   | LIM1 merged                     |
| —     | the non-breaking release                              | the main session, with the owner                        | LIM2 merged                     |
| LIM3  | #1059; its plan is written when LIM2 lands            | Opus for the plan; Sonnet stage agent                   | the non-breaking release is cut |
| LIM4  | garden this driver and the plans with `sdd-gardening` | the main session                                        | LIM3 merged                     |

The stages are sequential: LIM2 lowers the limit that LIM1 resolves, and LIM3
reads the column that LIM2 adds. LIM1 and LIM2 are additive (a unit with no
`[limits]` table behaves as today and keeps its catalog hash), so they ship in
the non-breaking release. LIM3 ships in the next release marked breaking.

## Stage briefs

### LIM1 — the manifest table and RIDL-151 (#1057)

The plan is `docs/wip/2026-10-24-lane-limits-lim1-plan.md`. One pull request,
`Closes #1057`.

### LIM2 — the limit in the IR, the hash and the descriptor (#1058)

- Record a limit on an interaction only when a `[limits]` entry applies, so a
  unit with no table keeps its catalog hash; pin that with a test over an
  existing corpus unit.
- The plan is `docs/wip/2026-10-24-lane-limits-lim2-plan.md`. The descriptor
  column moves to LIM3 (R-16).
- `ridl diff`: a changed, added or removed limit is breaking.
- One pull request, `Closes #1058`.

### LIM3 — the transport (#1059)

- Add the limit column to the descriptor's member table (`catalog.fbs` `Member`,
  `lower.rs` `member_of`, `describe.rs`), moved here from LIM2 by R-16, and read
  it in the transport's member table.
- Follow the issue body and the owner comment on #1059.
- Load the `[limits]` table of every materialized remote unit (R-24), which
  settles driftsys/ridl#1067.
- The plan is `docs/wip/2026-10-27-lane-limits-lim3-plan.md`.
- One pull request, `Closes #1059` and `Closes #1067`, with a `BREAKING CHANGE:`
  footer.

## Rules every stage follows

Worktree under `.claude/worktrees/` with `./bootstrap`; a reviewer per task
before the next task starts; `/review` then fix then pass 2 before merge; CI
runs the full gates (implementers run the focused crate tests only); never a
tag, a publish or a push to `main`; plain prose. A stage agent escalates every
ruling this driver does not cover to the main session, and does not hand back
until its pull request merges or a real blocker stops it.

## Rulings

Each ruling: what was decided — why — what it costs if wrong.

- **R-1** — A wrong value type in `[limits]` (a string, a negative number, a
  table) reuses MANI-015; the plan named no code. — MANI-015 is the existing
  hard error for a wrong shape. — A new code and a catalogue row if the owner
  wants a distinct one.
- **R-2** — An unknown key directly inside `[limits]` warns MANI-005 (a `limits`
  row in `KNOWN_KEYS`). — A misspelled `default` would otherwise leave a safety
  limit silently unapplied. — One more warning on manifests that carry such a
  key.
- **R-3** — A limit of 0 is accepted. — ADR-0030 decision 12 gives the range 0
  to 16,256. — A manifest meant to forbid an interaction would pass.
- **R-4** — The `[limits]` table of a workspace root is parsed and then dropped;
  only package manifests (standalone or member) store limits. — The plan says a
  root table does not apply to members. — None.
- **R-5** — Unit limits are a public field `LoadedWorkspace.unit_limits`, not an
  accessor method. — It matches the sibling field `backends`. — A later rename
  to a method.
- **R-6** — Remote (materialized) units get no limits entry. — A unit's table
  applies to that unit only and the plan does not cover remote units. — A remote
  unit with a `[limits]` table is not checked.
- **R-7** — The single check of the plan is split into `check_payload_limits`
  (in `design_lints/mod.rs`) and `limits::check`. — Matches the RIDL-150 layout.
  — None.
- **R-8** — Owner ruling 2026-10-24: a `[limits.interaction]` key is the shape's
  short name (the last segment, no package), then `.`, then the member, for
  declared interfaces and inline services. An inline service `demo.svc` is keyed
  `svc.member`; `demo.svc.member` and `pkg.Iface.member` are RIDL-152. Two
  shapes of one unit with the same short name make a key naming it ambiguous,
  reported as RIDL-152 with a message saying so (same code). — The owner chose
  it over keeping the dotted global name. — A separate code for the ambiguous
  case if the owner wants one.
- **R-9** — The RIDL-151 message names the key as written in the manifest
  (`Iface.sig`), not the package-qualified form of RIDL-150. — The reader copies
  the name into `ridl.toml`. — None.
- **R-10** — A `fixed` names its role as "value" in RIDL-151. — A `fixed` has
  one value and no request or reply. — None.
- **R-11** — A unit with a `[limits]` table and no interaction is still checked,
  so each of its `[limits.interaction]` entries is RIDL-152. — The entries name
  no interaction. — None.
- **R-12** — The limits check finds a unit's table by unit name; a key may name
  an interface of any package of the unit, and short-name ambiguity is computed
  across the whole unit. — The table belongs to the unit (decision 12), and a
  subdirectory package was otherwise never checked. — A key may match an
  interface in a package the author did not expect.
- **R-13** — For RIDL-151 and RIDL-152, a manifest error in a file no package
  owns blocks only the packages beneath that file's directory; RIDL-150 keeps
  blocking everywhere. — One member's bad `ridl.toml` stopped the checks of
  every other member. — RIDL-151 runs beside an unrelated manifest error.
- **R-14** — RIDL-152 is reported only when every package of the unit was
  examined. — A skipped package's interfaces are unknown, so a key could be a
  false positive. — An unmatched key goes unreported while the unit has an
  error.
- **R-15** — A payload above the ceiling under a declared limit draws both
  RIDL-150 and RIDL-151. — Each states a different rule. — Two diagnostics on
  one member.
- **R-16** — The descriptor's limit column moves from LIM2 to LIM3. — A new
  field on the generated `Member` struct of `ridl-descriptor` is a break that
  the `runtime-crates` semver check reports, and LIM2 ships in the non-breaking
  release; the transport, the column's only reader, lands in LIM3. — Between the
  two releases a descriptor carries no limits, which nothing reads yet.
- **R-17** — A payload that draws RIDL-151 still has its limit recorded in the
  checked IR; a test states it. — The limit is declared, and the build stops on
  the error anyway. — None.
- **R-18** — `annotate_limits` records limits for every package of a unit that
  has a `[limits]` table, computes short-name ambiguity over all packages of the
  unit, and does not skip blocked packages. RIDL-151 computes ambiguity over the
  examined packages only. — The IR holds the unit-wide rule of R-8; the two
  differ only in a build that already has errors. — A limit checked and a limit
  recorded can differ in such a build.
- **R-19** — `freeze/codegen-protocol.txt` gained one line for the new proto
  field `Decl.limit = 31`. The plan said no dump changes, but the freeze check
  fails without the line and the line is additive. — The dump lists the IR
  proto. — None; no approved break is involved.
- **R-20** — `ridl diff` compares the limit only for a pair of the same kind; a
  kind change is reported as `KindChanged` alone. — The kind change is already
  breaking and the old limit has no meaning for another kind. — A limit change
  that comes with a kind change is not shown.
- **R-21** — The remote units' `[limits]` tables stay unloaded (R-6), so a
  consumer that generates a remote unit's code hashes it with no limit, and its
  catalog hash differs from the owner's when the owner declares a limit. Not
  fixed in LIM2; the owner decides. — Loading a remote unit's table is a change
  to the manifest loader that LIM1 ruled out of scope. — Peers may refuse to
  attach over a hash that differs only by this; nothing reads limits at run time
  until LIM3, which must settle it before shipping.
- **R-22** — ADR-0014 decision 15, the catalog-descriptor design record,
  ADR-0030 decision 13 (who records the limit) and two book pages were edited in
  this stage. — Issue #1058 and ADR-0030 "Where the rules are stated" require
  the records to state the limit; two sentences became false. — None.
- **R-23** — PR #1065 keeps `Closes #1058` although the descriptor column of
  #1058 moved to LIM3 (R-16); the pass 2 review raised it. — The stage brief
  fixed the closing keyword. #1059 is commented to own the column. — #1058
  closes with the column undone if #1059 is not followed.
- **R-24** — Owner ruling 2026-10-27, settling driftsys/ridl#1067 and reversing
  R-6: LIM3 loads the `[limits]` table of every materialized remote unit, so a
  consumer records the same limits as the owner and the two catalog hashes
  agree. A remote unit's limits are recorded, not checked again on the consumer
  side. — The alternative, a hash that excludes limits, contradicts ADR-0030
  decision 13 and ADR-0014 decision 15. — A remote unit with a malformed
  `[limits]` table stops a consumer's build.
- **R-25** — Owner ruling 2026-10-27: the LIM3 pull request merges once both
  `/review` passes are done and CI is green, as LIM1 and LIM2 did; the next
  release from `main` is then marked breaking. — The breaking changes were
  accepted on driftsys/ridl#1059. — None; no tag and no publish follow.
- **R-26** — `with_max_message_bytes` returns one error,
  `LimitOutOfRange
  { given }`, for both ends of the range 294 to 16,292; it
  replaces `LimitTooSmall`. — `LimitTooSmall` is a false name for a value above
  the range, and one type is simpler than an enum of two. — A rename if the
  owner prefers two types.
- **R-27** — The constant is `frame::MAX_MESSAGE_LIMIT = 16_292`, written as a
  literal with a test that derives it from 36 + 16,256. — `ridl-rt` has no
  ceiling constant to compute it from. — None.
- **R-28** — `MemberTable` gains `set_limit` and `limit`; `insert` and `member`
  keep their signatures. — The member table's API does not break. — Two calls
  instead of one for a backend that sets a limit.
- **R-29** — An over-limit request is answered `corrupt` and records no `Note`.
  — A payload that fails to decode records none either. — An embedder cannot see
  the cause; an additive `Note` variant can follow.
- **R-30** — In `request`, a repeated `seq` is answered first, then an unknown
  member, then the length check. — ADR-0030 decision 15 does not fix the order,
  and this order keeps the existing answers. — None.
- **R-31** — `SCHEMA_VERSION` stays 1; the limit column is an optional field
  appended to `Member`. — An older reader ignores it, and ADR-0030 decision 15
  treats an absent column as "message limit alone". — None.
- **R-32** — `from_descriptor` stores any `u32` limit and refuses none above
  16,256. — The manifest cannot produce one. — A hand-built descriptor sets a
  limit the message limit already bounds.
- **R-33** — The retained-bytes minimum is the constant 16,292, not the
  configured message limit. — ADR-0030 decision 15 and #1059 state the constant.
  — None.
- **R-34** — The frame specification states 16,292 as the Rust provider's
  maximum and default only, not as a rule every receiver of the binding
  enforces. — ADR-0030 does not decide it. — The owner may make it a binding
  rule later.
- **R-35** — ADR-0030 is not edited; only `docs/decisions/README.md` changes.
  Whether the ADR's Status gains a dated "built" note is left to the owner. —
  The record's Status is the owner's. — None.
- **R-36** — The plan commit uses the scope `docs(roadmap)`, as the LIM1 and
  LIM2 plans did. — Consistency with the lane. — None.
