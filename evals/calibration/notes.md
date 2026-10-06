# Design check calibration: notes

The tables of the derivation, with the precision at every candidate threshold,
are in [`summary.md`](summary.md), which is the output of
`cargo xtask calibrate derive --write` unchanged. This file is written by hand
and records the method, the outcome applied to the compiler, and the recall
mapping with its judgement calls.

## How the labels were produced

- The 820 findings were dumped at `8ef2f28d`; a second dump at `200185f6`, the
  revision the calibration branch was then fast-forwarded to, was byte-identical
  for all five findings files. They were labelled blind by two labellers: Claude
  (`claude -p --model fable`) and Sol (`codex exec -m gpt-6.1-sol`). Neither saw
  the other's labels.
- Sol labelled through a rule-based script keyed on token, type and interface
  name, so Sol's labels are the script's output, kept as produced. Both
  labellers dismissed all 780 `inconsistent-abbreviation` findings.
- The two labellers disagreed on 8 findings: the one `duplicate-shape` finding
  `duplicate-shape:mavlink:common/messages.typl:25115-25132:0` (Claude accept,
  Sol dismiss) and all 7 `inconsistent-unit` findings (Claude accept, Sol
  dismiss). Sebastien adjudicated all 8 as `accept`. He also agreed with all 31
  sampled agreements (ten each for `duplicate-shape`,
  `inconsistent-abbreviation` and `low-cohesion-interface`, and the one
  available for `package-fan-out`), so every agreed label stands.
- Recall counts a matching finding whatever its label: labels measure precision,
  matches measure recall. Recall is reported and never selects or gates a
  threshold or a level.

## Outcome per check

| Check                       | Findings                      | Accepted                     | Threshold applied                                | Level        | Retained | Recall         |
| --------------------------- | ----------------------------- | ---------------------------- | ------------------------------------------------ | ------------ | -------- | -------------- |
| `inconsistent-unit`         | 7 (mavlink)                   | 7                            | none                                             | Info (ships) | 7        | not applicable |
| `inconsistent-abbreviation` | 780 (mavlink, ros2)           | 0                            | none                                             | dropped      | 0        | not applicable |
| `duplicate-shape`           | 16 (mavlink 2, ros2 7, vss 7) | 8 (mavlink 1, ros2 0, vss 7) | fields >= 2, variants >= 2                       | Info (ships) | 16       | 3/3            |
| `low-cohesion-interface`    | 16 (mavlink, ros2, vss)       | 1                            | at least 7 groups, minimum group size at least 1 | Info (ships) | 2        | 1/1            |
| `package-fan-out`           | 1 (ros2)                      | 0                            | fan-out > 3                                      | dropped      | 0        | not applicable |

- `inconsistent-unit`: precision 7/7 = 100 % at its only candidate. Fewer than
  ten findings caps the level at Info. Ships at Info with no threshold.
- `inconsistent-abbreviation`: precision 0/780 = 0 %, below the 50 % floor. Not
  shipped; the catalogue row, code TYPL-223 and the emitter are removed.
- `duplicate-shape`: the least strict candidate (at least 2 fields and at least
  2 variants) has precision 8/16 = 50.00 %, exactly the floor, so it ships at
  Info with 16 retained findings. No candidate reaches Warning: the candidates
  at or above the 80 % bar (at least 4 fields with at least 2 variants, 8/8; at
  least 4 fields with at least 4 variants, 1/1; at least 5 fields with at least
  2 variants, 7/7) retain fewer than ten findings, which caps them at Info, and
  the least strict candidate that qualifies for Info is selected. The candidate
  with at least 3 fields and at least 2 variants (8/13 = 61.54 %) also qualifies
  for Info but is stricter. The 8 dismissed findings are the 7 ros2 findings,
  where both labellers read the equal shapes as types that upstream keeps
  distinct on purpose (time against duration, velocity against acceleration,
  point against vector, per-action results, per-service responses), and 1
  mavlink finding.
- `low-cohesion-interface`: at the search start (at least 2 groups, with a
  minimum group size of at least 1) precision is 1/16 = 6.25 %. The least strict
  pair that reaches the floor is at least 7 groups with a minimum group size of
  at least 1, with 2 retained findings: 1 accepted (`Nav2MsgsInteractions` in
  ros2, 17 groups, finding
  `low-cohesion-interface:ros2:nav2_msgs/interactions.ridl:15158-15178:0`) and 1
  dismissed (`MissionProtocol` in mavlink `common/interactions.ridl`, 8 groups,
  finding `low-cohesion-interface:mavlink:common/interactions.ridl:61-76:0`).
  Fewer than ten findings caps the level at Info. Ships at Info at that pair.
- `package-fan-out`: precision 0/1 = 0 %. Not shipped; the catalogue row, code
  RIDL-415 and the emitter are removed. The fan-in, fan-out and instability
  metrics stay available through `ridl_metrics`.

## Recall mapping

The reviewed join is `evals/calibration/recall.toml`. Four rubric issues are
applicable to a check; every other issue is inapplicable with a reason, and no
issue concerns an abbreviation, a unit conflict between names or a package
dependency count, so three checks report `not applicable`.

| Check                    | Rubric issue                                                     | Matched findings                                                        | Label of the match |
| ------------------------ | ---------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------ |
| `duplicate-shape`        | `review-0001:1` (Point and Vector3 share x/y/z)                  | `duplicate-shape:ros2:geometry_msgs/messages.typl:7423-7430:0`          | dismiss            |
| `duplicate-shape`        | `review-0003:1` (missionRequest and missionRequestInt)           | `duplicate-shape:mavlink:common/messages.typl:25115-25132:0`            | accept             |
| `duplicate-shape`        | `review-0005:1` (station packages repeat declarations)           | the seven vss `AirDistribution` findings                                | accept             |
| `low-cohesion-interface` | `review-0002:1` (Nav2MsgsInteractions combines responsibilities) | `low-cohesion-interface:ros2:nav2_msgs/interactions.ridl:15158-15178:0` | accept             |

Two judgement calls in this mapping were noted in review:

- Counting `review-0003:1` for `duplicate-shape` is a broad reading: the rubric
  item is about the return type of the two request calls, not about the two
  request structs having the same fields.
- Classing `review-0001:1` as an issue is a judgement call: the rubric asks a
  review to identify the shared shape, and item 6 of the same rubric forbids
  treating that shape alone as proof that either declaration should be removed.

Under the stricter readings, `duplicate-shape` recall at the shipped threshold
is 2/2 (without `review-0003:1`) or 1/1 (without both). Recall never selects or
gates a threshold or a level, so nothing shipped depends on these readings.

## Counts pinned on the corpus

`evals/calibration/expected-counts.toml` pins, per workspace, the findings the
three shipped lints report at default levels: `duplicate-shape` 2 (mavlink), 7
(ros2), 7 (vss); `inconsistent-unit` 7 (mavlink); `low-cohesion-interface` 1
(mavlink), 1 (ros2).
