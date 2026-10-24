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
- Add the descriptor column as an addition under ADR-0029; update the freeze
  dumps with `cargo xtask freeze update`.
- `ridl diff`: a changed, added or removed limit is breaking.
- One pull request, `Closes #1058`.

### LIM3 — the transport (#1059)

- Follow the issue body and the owner comment on #1059.
- One pull request, `Closes #1059`, with a `BREAKING CHANGE:` footer.

## Rules every stage follows

Worktree under `.claude/worktrees/` with `./bootstrap`; a reviewer per task
before the next task starts; `/review` then fix then pass 2 before merge; CI
runs the full gates (implementers run the focused crate tests only); never a
tag, a publish or a push to `main`; plain prose. A stage agent escalates every
ruling this driver does not cover to the main session, and does not hand back
until its pull request merges or a real blocker stops it.

## Rulings

Each ruling: what was decided — why — what it costs if wrong.

(none yet)
