# Design lints and the eval seed — design for piece 1b

Status: design spec for piece 1b of
[`2026-10-03-devex-and-agent-tracks-brief.md`](2026-10-03-devex-and-agent-tracks-brief.md),
written 2026-10-04 against `main` at c1e63351. Sebastien agreed the approach in
the brainstorming session of 2026-10-04 (decisions D-1 to D-8, §2), and approved
the written spec, D-9 included, the same day. It is archived with its plan when
the work lands.

Satisfies: the brief's piece 1b, and the first step of piece 1c ("start
collecting real design and review tasks as 1c's eval set"); ADR-0005 §5 (evals
are part of the deliverable).

Related: [ADR-0024](../decisions/ADR-0024-lint-registry-and-levels.md) (every
new check is a lint under its decision 7),
[ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md) (the new tool is
additive under its decision 9),
[ADR-0005](../decisions/ADR-0005-agent-enablement.md) (eval scoring beyond
"compiles" stays its open question; 1b fixes the task format only).

Out of reach for this spec: the codes TYPL-406 to TYPL-410 and the record number
ADR-0026, both held by piece 2a.

## 1. Goal and success criteria

The brief names the problem: the design metrics are easy to compute, but a
default-on lint that reports findings a designer would dismiss teaches the
designer to ignore every lint. 1b answers two questions with evidence:

1. Which design checks become lints, and at which default level.
2. For a check with a numeric threshold, which threshold.

The evidence is a corpus of real interface sets, designed by other people for a
real purpose, ported into RIDL. The same corpus is the starting point of 1c's
eval tasks, so 1b also seeds the eval set and fixes its format.

1b is done when:

- the corpus of §3 is committed, compiles without an error, and stays inside its
  size budget, enforced by a test;
- every candidate check of §4 is implemented, and its findings on the corpus are
  labelled and adjudicated (§7);
- each check has a default level, and each threshold a value, derived from those
  labels by the rule of D-5, and the derivation is committed;
- a test pins the number of findings each lint reports on each corpus workspace;
- `ridl_metrics` (§6) is served by `ridl mcp`;
- about ten eval tasks (§8) are committed in the agreed format, and a test
  checks that each one parses and names an existing corpus workspace.

## 2. Decisions

- **D-1. The evidence is a port of public interface sets.** GPT Sol 6.1 ports
  them, driven non-interactively through `codex exec` from this session, from
  the brief in §3. Claude reviews every port against its upstream before it is
  committed. Private workspaces are not used.
- **D-2. Three sets, automotive at most one third.** A ROS 2 subset (robotics),
  a MAVLink `common` subset (drones and autopilots), and a partial COVESA VSS
  (automotive). The budget is 6,000 lines of ported source in total: at most
  2,500 lines each for ROS 2 and MAVLink, and at most 1,000 lines for VSS. The
  directory names are domain-neutral, and nothing outside `evals/` names a
  domain.
- **D-3. Everything is public, in this repository, under a top-level `evals/`.**
  It is outside `crates/`, so no published crate and not the book ships it. A
  private hold-out set for 1c, if 1c's scoring needs one, is 1c's decision;
  adding one later removes nothing from here.
- **D-4. Findings and metrics are separated.** A finding is a yes-or-no fact
  about one place. A metric is a number about a package or an interface, and
  needs a threshold to become a finding. The candidate checks are those of §4.
  Package import cycles are not a 1b check: "circular package imports" is
  already an Error (typl §16.1, ADR-0002 §6). Naming consistency covers
  abbreviations only; a synonym vocabulary (`speed` against `velocity`) is left
  to the skill's judgement, because it would be a curated list the project has
  to maintain.
- **D-5. A check's default level comes from its precision on the corpus.**
  Precision is the share of a check's corpus findings that the adjudicated
  labels accept.
  - Precision of at least the bar (80 %): the catalogue row is a Warning, so its
    default level is `warn`.
  - Precision from 50 % up to the bar: the catalogue row is an Info, default
    level `info`. A project can raise it to `warn` or set it to `allow`.
  - Precision below 50 %: the check does not ship as a lint. A metric check
    stays available through `ridl_metrics` (§6).

  A threshold is fixed in code, as a constant beside the check, as
  `SHARED_ERROR_INTERFACE_THRESHOLD` already is for RIDL-405
  (`crates/ridl-sem/src/lint.rs`). Its value is the least strict value whose
  precision still meets the level it ships at. A threshold is not configurable
  in `ridl.toml`; ADR-0024 decision 2 sets levels only. The 80 % bar and the 50
  % floor are parameters of this procedure: changing either one re-runs §7.4
  over the committed labels, with no new labelling.
- **D-6. Labels are double-blind and adjudicated.** Claude and Sol each label
  every finding _accept_ or _dismiss_, with a one-line reason, without seeing
  the other's labels. Where they agree, the label stands. Where they disagree,
  Sebastien decides. All three columns are committed.
- **D-7. A new read-only MCP tool, `ridl_metrics`, returns the metrics**, so the
  1c skill can cite the numbers for every check, whatever its level. It is an
  addition to the tool surface under ADR-0025 decision 9.
- **D-8. An eval task is a directory with three files** (§8): `task.toml` for
  the machine-checkable expectations, `prompt.md` for what the designer asks,
  `rubric.md` for what a good answer contains. The seed is about ten tasks,
  weighted toward review and evolution, as the maintainer weighted them.
- **D-9. Review rubrics are written before the checks run.** The review tasks'
  rubrics list the real design issues of a corpus workspace. Written before
  anyone sees the lint findings, they are an independent reference: they give
  each check a recall figure (which listed issues it finds) beside its
  precision, and they keep the labels in §7 from being anchored on the checks'
  own output. Recall is reported, not gated.

## 3. The corpus and the porting brief

### 3.1 Layout

```text
evals/
  README.md                 what evals/ is, the formats, how to add a task
  corpus/
    ros2/                   one RIDL workspace per set
      ridl.toml
      PROVENANCE.md         upstream, revision, licence, subset, port choices
      LICENSE               the upstream licence text, byte-exact
      <package dirs>/
    mavlink/
    vss/
  calibration/
    <lint-name>.toml        the labelled findings of one check (§7.3)
    expected-counts.toml    findings per lint per workspace, at default levels
  tasks/
    <id>/
      task.toml
      prompt.md
      rubric.md
```

Each upstream `LICENSE` file is listed in `.primignore`, so `prim fmt` does not
rewrite it. `THIRD-PARTY-NOTICES.txt`, the existing notices file, gets one entry
per set: the upstream project, its licence, and the path of the port.

### 3.2 The sets

| Set       | Upstream                                                                | Subset                                                                                                                                                       | Budget        |
| --------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------- |
| `ros2`    | `ros2/common_interfaces` and `ros-navigation/navigation2` (`nav2_msgs`) | `std_msgs`, `geometry_msgs`, `sensor_msgs`, `nav_msgs`, `std_srvs`, `nav2_msgs`: real dependencies between packages, services and actions, units in comments | ≤ 2,500 lines |
| `mavlink` | `mavlink/mavlink`, `message_definitions/v1.0/common.xml`                | The heartbeat and system status messages, and the mission, command, parameter and telemetry microservices                                                    | ≤ 2,500 lines |
| `vss`     | `COVESA/vehicle_signal_specification`                                   | The `Vehicle.Cabin.HVAC` branch, and `Vehicle.Powertrain.TractionBattery` if the budget allows                                                               | ≤ 1,000 lines |

The upstream revision is pinned in each `PROVENANCE.md`. Before porting, the
licence of each upstream is verified. ROS 2 and nav2 are expected to be
Apache-2.0 and VSS MPL-2.0. MPL-2.0 is a file-level licence, so the ported VSS
files stay MPL-2.0 inside this MIT repository, and `evals/corpus/vss/LICENSE`
says so. If the MAVLink message definitions are not under a permissive licence
(MIT, BSD, Apache-2.0) or MPL-2.0, the set is replaced by the AOSP sensors and
power HAL AIDL interfaces (Apache-2.0, not automotive), with the same budget.

### 3.3 Porting rules

These rules are the brief Sol receives. They exist so that the port keeps the
upstream design as it is, defects included, because the defects are the
evidence.

1. **Keep every upstream name, unit, grouping and comment.** Do not rename,
   merge, split, or correct anything, even where it looks wrong. An identifier
   is changed only where RIDL's lexical rules require it (case convention,
   reserved word), and each such change is listed in `PROVENANCE.md`.
2. **A unit is written only where upstream states it**: MAVLink's `units`
   attribute, VSS's `unit` field, or a ROS 2 field comment that names a unit.
   The unit is mapped to its UCUM form; a unit outside the curated UCUM atom
   table (`crates/ridl-sem/src/ucum.rs`) is left off and listed.
3. **An interface boundary is drawn only where upstream documents one**: one ROS
   2 package's services and actions form one interface; one MAVLink microservice
   forms one interface; one VSS branch node forms one interface.
4. **Interaction kinds follow a fixed rule per set**, written at the top of that
   set's `PROVENANCE.md` before the port starts:
   - ROS 2: a service is a `query` (request fields as parameters, the response
     as a returned struct); an action is a `command` for the goal and an `event`
     for its feedback.
   - MAVLink: a message documented as streamed telemetry is a `signal`; a
     request and its acknowledgement follow the microservice's documented
     protocol (`command` or `query`); any other message is an `event`.
   - VSS: a `sensor` or an `attribute` is a `signal`; an `actuator` is a
     `signal` plus a `command` that sets it.

   A case the rule does not decide is decided by the porter and listed.
5. **Packages follow upstream packages**; VSS branches map to one package each.
6. **Leave out, do not stub.** A message, field or branch outside the subset is
   omitted. A type the subset needs from outside it is ported too, and listed.
7. The result must check with no Error diagnostic. Warnings are expected and are
   not fixed.

Each `PROVENANCE.md` records: the upstream URL and revision, the licence, the
subset, the kind rule, and every deviation from rules 1 to 6 with its reason.

### 3.4 Review of a port

Before a port is committed, Claude checks it against its upstream: every
declaration of the subset is present; a sample of 20 declarations per set is
compared field by field; every listed deviation is justified. A port that fails
the review goes back to Sol with the findings.

## 4. The candidate checks

All five run in one workspace-level pass (§5). "The workspace" is what
`ridl
check` loads from its entry point. Types of the standard package
`ridl.std` are excluded from every check below.

### 4.1 `inconsistent-unit` (finding)

For every struct field, every interaction parameter, and every interaction
payload field in the workspace, take the pair (name, unit), where the unit is
the canonical UCUM form of the unit type the declared type resolves to, through
aliases. A site whose type has no unit is skipped. When one name occurs with two
or more distinct units, each site whose unit is not the most frequent unit of
that name is reported; on a tie, every site is reported. The message names the
other units and one location of each.

Names are compared exactly. The UCUM parser has no dimension analysis today, so
the check does not tell a different scale (`m/s` against `cm/s`) from a
different dimension (`m/s` against `K`); that distinction is deferred.

### 4.2 `inconsistent-abbreviation` (finding)

Split every declared identifier in the workspace (types, fields, enum variants,
interfaces, interaction members, parameters) into lower-cased words at case
boundaries, digits and underscores. A word `a` is reported as an abbreviation of
a word `b` when `a` is a strict prefix of `b`, `a` has at least 3 letters, `b`
has at least 2 more letters than `a`, and both occur in the workspace (`temp`
and `temperature`, `pos` and `position`). Each identifier that uses `a` is
reported, naming `b` and one identifier that uses it. There is no dictionary and
no exception list; calibration decides whether the rule is precise enough.

### 4.3 `duplicate-shape` (finding with a size threshold)

Two struct declarations with different qualified names whose fields are equal as
sets of (name, resolved type), with at least `DUPLICATE_SHAPE_MIN_FIELDS`
fields, are reported. Two enum declarations with equal sets of variant names,
with at least `DUPLICATE_SHAPE_MIN_VARIANTS` variants, are reported the same
way. The later declaration, by package name and then source order, is reported,
naming the earlier one. The calibration search starts at 2 for both thresholds.

### 4.4 `low-cohesion-interface` (metric)

For each declared `interface`, give each member the set of named types it
references directly: its payload, its parameters, its return type and its error
type, excluding primitives and `ridl.std`. A member with an empty set is left
out. Members that share a type are linked, and the linked groups are counted
(the LCOM4 measure). An interface is reported when it has at least
`LOW_COHESION_MIN_GROUPS` groups and its smallest group has at least
`LOW_COHESION_MIN_GROUP_SIZE` members. The message lists the groups. The search
starts at 2 groups and a group size of 1.

### 4.5 `package-fan-out` (metric)

The number of distinct workspace packages a package depends on, with the
dependency edges `ridl_dependencies` already reports (ADR-0025 decision 8:
imports, and rsdl component uses), excluding `ridl.std`. A package whose fan-out
is above `PACKAGE_FAN_OUT_MAX` is reported at the `package` line of its first
file in source order. The search starts at 3. Fan-in and instability are metrics
only (§6): a package many others use is normal, not a defect.

## 5. Placement and catalogue

- **One pass, in `ridlc`, after the per-package checks and before
  `apply_lint_levels`.** The checks need the whole checked workspace; the
  existing per-package lints in `ridl-sem/src/lint.rs` see one package. The pass
  emits each code at its catalogue severity, so ADR-0024's level handling
  applies with no change. It runs wherever the shared compile runs, including
  `check_source`, where a workspace is one package.
- **The dependency edges are computed once**, by the function
  `ridl_dependencies` uses, moved into `ridlc` if it lives in `ridl-mcp` today,
  so that `package-fan-out`, `ridl_dependencies` and `ridl_metrics` agree.
- **Codes.** A check about the type vocabulary (`inconsistent-unit`,
  `inconsistent-abbreviation`, `duplicate-shape`) takes a TYPL-2xx code; a check
  about interfaces or coupling (`low-cohesion-interface`, `package-fan-out`)
  takes a RIDL-4xx code, beside RIDL-404 and RIDL-405. The numbers are taken at
  implementation time from the first free codes, after checking open pull
  requests for claims: TYPL-221 is claimed by the portable-patterns work, and
  TYPL-406 to TYPL-410 by 2a. A check that ends below the 50 % floor never gets
  a code.
- **During calibration**, every check is a catalogue row at Info, so that it
  runs and reports. Its final severity is set by §7.4.
- **ADR-0024 decision 7 applies to each shipped lint**: its name on the
  catalogue row, a row on [the lints page](../book/lints.md#the-lints), and the
  expected list in the `ridl-core` diagnostic test, in the same change. A check
  dropped after calibration is removed before release, so its name was never
  released and nothing is reserved.

## 6. `ridl_metrics`

Input: `{path}`, resolved as every path tool resolves it (ADR-0025 decision 2).
Output:

```json
{
  "packages": [
    { "name": "nav2_msgs", "fanIn": 0, "fanOut": 3, "instability": 1.0,
      "dependsOn": ["geometry_msgs", "nav_msgs", "std_msgs"] }
  ],
  "interfaces": [
    { "name": "nav2_msgs.Navigation", "members": 7,
      "groups": [["navigateToPose", "followPath"], ["clearCostmap"]] }
  ],
  "workspace": { "root": "...", "notes": [] }
}
```

Instability is fan-out divided by fan-in plus fan-out, and is `null` for a
package with neither. The values are exactly those §4.4 and §4.5 compute; the
tool reports them for every package and interface, with no threshold. The
`workspace` object is the one the other path tools return. The tool is read-only
and offline (ADR-0025 decision 6), and is added to
`crates/ridl-mcp/tests/tools.json`.

## 7. Calibration

### 7.1 Order

1. The ports (§3) are committed.
2. The review rubrics (§8) are written for the corpus workspaces (D-9).
3. The checks (§4) are implemented as Info rows.
4. The findings are dumped, labelled, adjudicated, and the levels and thresholds
   derived.

Steps 2 and 3 can run in parallel; step 4 needs both.

### 7.2 Dumping the findings

An `xtask` command, `cargo xtask calibrate`, copies each corpus workspace to a
temporary directory, appends a `[lints]` table that sets every candidate check
to `warn`, runs the check with each threshold at its search start, and writes
one JSON file of findings per check. No `ridl` command or flag is added for
this. For a metric check, it also writes the metric value of each finding, so
that one labelling serves every candidate threshold.

### 7.3 Labelling

Claude and Sol each receive the findings files and the corpus, and nothing else:
not the other's labels, not this spec's expectations. Each writes, per finding,
`accept` or `dismiss` and a one-line reason. The question each answers is:
"Would a designer reviewing this workspace change the design because of this
finding?" The merged file is `evals/calibration/<lint-name>.toml`:

```toml
[[finding]]
workspace = "mavlink"
location = "mission/mission.ridl:41"
message = "field `alt` uses `m`; elsewhere `alt` uses `mm` (telemetry/position.ridl:12)"
metric = 2         # metric checks only
claude = "accept"
claude_reason = "..."
sol = "dismiss"
sol_reason = "..."
final = "accept"   # set by Sebastien where claude and sol disagree
```

### 7.4 Derivation

`cargo xtask calibrate --derive` reads the labels and prints, per check, the
precision at each candidate threshold, the recall against the review rubrics,
and the level and threshold D-5 gives. The output is committed as
`evals/calibration/summary.md`, and the levels and thresholds are written into
the catalogue and the constants by hand, in the same change.

### 7.5 The guard

A test in `crates/ridl/tests/` checks each corpus workspace at default levels
and compares the number of findings of each 1b lint with
`evals/calibration/expected-counts.toml`. A change to a check or to the language
that changes a count fails the test, and its author updates the file
deliberately, so the change is visible in review. The same test checks that each
corpus workspace has no Error diagnostic and that the line budgets of D-2 hold.

## 8. The eval seed

### 8.1 Task format

`task.toml`:

```toml
id = "review-0003"
kind = "review"          # review | evolve | design
corpus = "mavlink"       # review and evolve; absent for design
title = "Review the mission microservice"

[expect]
compiles = true          # the answer's RIDL checks with no Error
lints = ["inconsistent-unit"]   # review: lints a good answer cites as evidence
diff = "compatible"      # evolve: the ridl_diff category the change should have
```

`prompt.md` is what a designer asks the assistant, written as the designer would
write it, with no hint of the rubric. `rubric.md` lists numbered items, each one
sentence and each marked **must**, **should** or **must not**, that a reader can
check against an answer without consulting the author.

### 8.2 The seed

About ten tasks, spread over the three sets with at most a third on `vss`:

- five **review** tasks, one or two per corpus workspace, whose rubrics are the
  reference of D-9;
- two **evolve** tasks: a change request on a corpus workspace, with the
  `ridl_diff` category the change should have;
- three **design** tasks: a short written requirement, paraphrased from the
  upstream project's public documentation, and the rubric of a good RIDL design
  for it.

Claude drafts each task, Sol reviews it independently, and Sebastien approves
the set. No harness and no scoring are built in 1b: those are 1c's, and
ADR-0005's open question on scoring beyond "compiles" stays open.

### 8.3 The test

A test checks that each `task.toml` parses, that its `corpus` names a directory
of `evals/corpus/`, that its `lints` name lints in the catalogue, and that
`prompt.md` and `rubric.md` exist and are not empty.

## 9. Documentation

- The lints page gets a row for each shipped lint, and a short section saying
  how the design lints' levels were set, pointing at
  `evals/calibration/summary.md`.
- `evals/README.md` describes the corpus, the task format and the calibration
  procedure.
- The MCP crate README and the design record
  [`mcp-workspace-tools.md`](../design/mcp-workspace-tools.md) list
  `ridl_metrics`.
- Gardening decides whether the precision rule of D-5 is recorded as an ADR (the
  next free number after ADR-0026) or as an amendment to ADR-0024. It binds
  every later design lint, so it is normative.

## 10. Alternatives considered

- **Private workspaces as evidence.** Most realistic, but they cannot be
  committed or named, so nobody else can rerun the evidence. Rejected (D-1).
- **A private repository for the corpus and the tasks.** Protects the rubrics
  from training data, but moves the guard test out of this repository's CI, so a
  lint change that misfires on real designs would not fail its own pull request.
  Rejected for 1b; a private hold-out stays open to 1c (D-3).
- **Writing new workspaces by hand.** Fully controlled, but designed by the same
  people who set the thresholds, which is the bias the corpus exists to avoid.
- **Configurable thresholds in `[lints]`** (`{ level = "warn", max = 8 }`). More
  flexible, but it changes the manifest schema under ADR-0002 and makes every
  parameter a released contract. Left as a follow-up for a project that asks for
  it.
- **Metrics only through a tool, never as lints.** Matches the maintainer's
  split most literally, but then no CI ever reports a coupling problem. Kept as
  the outcome for a metric below the floor, not as the rule (D-5).
- **An off-by-default catalogue level** (Clippy's pedantic group). Would let a
  noisy check ship and be opted into, but needs an amendment to ADR-0024 and its
  catalogue guard. The Info level serves the 50 % to 80 % band instead.
- **A synonym vocabulary for naming.** Catches `speed` against `velocity`, at
  the cost of a curated list. Left to the skill (D-4).
- **Package cycles as a 1b check.** Already an Error.

## 11. Risks

- **The porter's choices shape the evidence.** The kind rules of §3.3 and the
  boundary rule fix most choices in advance, and every remaining one is listed,
  but the cohesion check in particular measures interfaces the porter grouped.
  The calibration summary states, per check, which sets its findings come from.
- **A small corpus gives few findings per check.** A check with fewer than ten
  findings on the corpus gets its level from too little data; the summary marks
  it, and it ships at Info at most.
- **The labellers share a bias.** Two models can agree on a wrong label.
  Sebastien sees only the disagreements; a sample of ten agreed labels per check
  is also shown to him.
