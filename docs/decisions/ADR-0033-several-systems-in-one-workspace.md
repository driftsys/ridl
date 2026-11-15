# ADR-0033 — Several systems in one workspace

## Status

Proposed — 2026-11-15. Scope: how many `system` declarations a workspace may
hold, how a `deployment` and a command select one of them, what the lowered IR
carries per system, how `ridl diff` compares systems, and what the instance lock
and the frozen surfaces become. It binds the rsdl reference, ADR-0022, the
checker in `ridl-sem`, `ridlc`, the `ridl` CLI and `ridl-diff`.

Written for driftsys/ridl#1397, roadmap story E6.21 (Epic 6). It retires one
diagnostic code (RSDL-601), adds one (RSDL-605), adds one CLI flag (`--system`)
and adds no syntax. The maintainer resolved its five questions on 2026-11-15
(see "Questions resolved"). It stays Proposed because it is not implemented, and
the retirement of RSDL-601 still needs its ADR-0029 amendment when the work is
scheduled. The documents in "Documents amended" are changed with the acceptance,
not before.

## Context

rsdl §1.5 says a workspace declares at most one `system`; a second is RSDL-601
(§2, §3.1). The checker raises RSDL-601 in `closure()` in
`crates/ridl-sem/src/rsdl/closure.rs`, reads the closure of the first system
only, and `placement.rs` treats RSDL-601 as blocking every deployment.
`WorkspaceOutput::system` in `crates/ridlc/src/lib.rs` is an
`Option<ridl_ir::v2::System>`, `ridl build` selects a deployment with
`--deployment` (`select_deployment`), and `ridl diff` compares the one lowered
system of each side (`crates/ridl/src/main.rs`).

The maintainer raised two cases on 2026-11-14 (ROADMAP E6.21):

- one workspace that describes two systems which deploy the same interfaces and
  reuse some component declarations;
- a system that does not need a unit of its own.

Today the workaround is a second workspace, or a unit that holds only the
system. ADR-0002 already lets one unit hold contracts and a system (the
paragraph "Alternatives rejected for the unit" in its §1 cites
`examples/cabin`), so the second case is blocked by RSDL-601 and not by the unit
model: once a second unit's packages are in the workspace, a second system
anywhere in it is an error.

Several facts already scale to several systems without change:

- the system artifact is `<pkg.Name>.system.{json,txtpb,binpb}` (ADR-0022
  decision 2), qualified by package and name;
- deployment names are unique in the workspace (RSDL-708), and the descriptor
  file stems are unique without case in the workspace (RSDL-711);
- the instance lock `instances.lock` is keyed by the component's qualified name
  and the instance name (rsdl §7, E19.2), not by the system.

## Decision

1. **RSDL-601 is retired. A workspace declares any number of systems, in any
   package of any unit.** Two systems in one package need different names
   (TYPL-009, as for any declaration). A system needs no unit of its own: it may
   sit in a package beside contracts, in a unit that holds other systems, or in
   its own unit, as the author chooses. ADR-0002 is not amended. Each system has
   its own closure; every completeness check of rsdl §8, §9 and §3.3 quantifies
   over the closure of the system it concerns. A component may be listed by
   several systems. The visibility rule of §2 ("every declaration it names has
   to be visible from the system's package") holds per system. RSDL-601 moves to
   §16.3 "Retired" and is not reused. _Rationale:_ the one-system rule protected
   the single `system` slot in the IR and the CLI, not a property of the
   language; decisions 2 to 6 replace that slot with a selection.

2. **A `deployment` selects its system with `for`, as written today.** `for`
   names one declared system, bare (resolved like any other name, typl §3.2) or
   qualified; a name that resolves to no system stays RSDL-704. A deployment
   belongs to exactly one system, and its RSDL-7xx checks run against that
   system's closure. Deployment names stay unique across the whole workspace
   (RSDL-708), whatever system they are `for`. A workspace with one system
   behaves exactly as today: the same source checks, lowers and builds with the
   same output and with no new flag. _Rationale:_ `for` was already in the
   grammar and already resolved a name; only its "names the workspace's system"
   wording assumed one system.

3. **`ridl build`, `ridlc build` and `ridl diff` take `--system <NAME>`.** The
   value is a system name, bare or qualified (`veh.Vehicle`). A bare name
   matches the systems of that declared name; when it matches more than one, the
   run fails and lists the qualified names.
   - `ridl build` and `ridlc build`: with neither `--system` nor `--deployment`,
     every system is checked and lowered and every system's artifacts are
     written. `--system` narrows the build to that system. `--deployment`
     narrows it to that deployment, and so to its system (deployment names are
     unique, decision 2). Both flags together must agree. With one system, the
     run is the same as today.
   - `ridl check` takes no flag and checks every system.
   - An unknown, ambiguous or disagreeing `--system` is a bad flag under
     ADR-0010: exit 2, a message that lists the declared systems by qualified
     name, nothing written. It is not a diagnostic and has no code, the same as
     an unknown `--deployment` (`UnknownDeployment`) today.
   - `--deployment` keeps its rules (`select_deployment`); without it, the
     codegen request carries a deployment section only when the selected system,
     or the workspace when no system is selected, declares exactly one
     deployment. _Rationale:_ ADR-0010 makes a selection a flag and a wrong flag
     exit 2; one flag spelled the same on the three commands keeps the CLI
     uniform.

4. **The IR carries one lowered system per system.** `WorkspaceOutput::system`
   becomes `systems`, a list of `ridl_ir::v2::System` ordered by qualified name,
   one per system whose closure has no error; a closure error blocks its own
   system only. `declared_deployments` records each deployment's system beside
   its name. `ridl build --emit ir-*` writes one `<pkg.Name>.system.*` artifact
   per lowered system; the names are already qualified, so no artifact name
   changes. Without `--system`, every system's artifact is written; with it,
   only that system's. The deployment descriptor names (`<stem>.deployment.*`,
   rsdl §3.4) do not change. RSDL-711 keeps its workspace scope: two deployments
   of different systems whose stems are equal without case still clash, because
   `--emit deployment` writes every descriptor of a build into one output
   directory. Two systems whose artifact names are equal when case is ignored
   (`a.Vehicle` and `a.vehicle`) are RSDL-605, a new error, case-blind like
   RSDL-711, reported on the later system and blocking that system only, because
   macOS and Windows file systems would write one file. The codegen request's
   deployment section is taken from the selected system. The message `System`
   itself does not change. _Rationale:_ each system is lowered by the code that
   lowers one today; only the container changes.

5. **`ridl diff` compares systems by qualified name.** The contract comparison
   over every package is unchanged (ADR-0022 decision 9). Then each system on
   both sides is compared under the rsdl §14 headings, and the report groups
   those headings under the system's qualified name. A system on one side only
   is listed under "composition changed" as "system added" or "system removed",
   with no verdict; its instances are not listed under "instance number
   changed", as an instance on one side only is not today. When each side has
   exactly one system, the two are compared whatever their names, so a renamed
   single system is reported as today and not as removed and added. `--system`
   restricts the system comparison to that name; it never restricts the contract
   comparison. Without `--system`, every system is compared. _Rationale:_ a
   report compares everything by default, and restricting the contract
   comparison would hide a breaking change (ADR-0022 decision 9). A side that is
   an `.ir.json` snapshot carries no system, so, as ADR-0022 decision 9 states,
   no system change is listed for it: its systems are not reported as added or
   removed. This departs from the issue's wording that `ridl diff` needs
   `--system` when the workspace has several systems; the maintainer accepted
   the departure on 2026-11-15 (question 2).

6. **One instance lock and the existing frozen surfaces, not one per system.**
   The workspace keeps one `instances.lock` beside the root `ridl.toml` (rsdl
   §7), with the same keys. An instance of a component listed by several systems
   has one number in all of them. A lock entry is live when its instance is in
   the closure of any system; RSDL-314 then means "in no system". `ridl lock`
   numbers the union of the closures. The freeze surfaces (ADR-0029,
   `freeze/surfaces.toml`) are tool surfaces, not per-system ones, and none is
   added. The `cli` dump gains `--system`, which is additive. Retiring RSDL-601
   removes the line `RSDL-601 error` from `freeze/diagnostics.txt`, because a
   retired code is not declared (`crates/ridl-core/src/diag.rs`); `diagnostics`
   has a baseline row in `freeze/surfaces.toml`, so this is a break of the
   `diagnostics` surface and `just freeze-check` fails on it. It needs an
   amendment of ADR-0029 under its decision 4, as RIDL-150's retirement had.
   `accepted-sources` does not change unless a frozen copy of a source with two
   systems is added under `freeze/` (`freeze/README.md`). _Rationale:_ an
   instance number addresses an instance of a component, and a client of a
   shared component must address it the same way in every system; per-system
   lock files would let two systems give one instance two numbers.

7. **E19.8 (variants) stays separate and may build on this.** A debug or staging
   variant can be written as a second system that lists the production
   components plus diagnostic ones, with its own deployments. Decision 6 already
   gives each shared instance the same number in both, which is E19.8's done
   criterion. Whether E19.8 needs more — for example a system written as an
   extension of another — is E19.8's decision, not this record's. _Rationale:_
   E6.21 is about independent systems; it should not decide the variant surface.

## Alternatives considered

- **Keep one system per workspace; a second system needs a second workspace.**
  The current workaround. Rejected: a component shared by two systems would be
  declared twice or published as a remote unit, and the two workspaces would
  allocate instance numbers independently.
- **A manifest `[systems]` table that names the active system.** Rejected: the
  system is already a declaration with a name, and the selection belongs on the
  command line (ADR-0010); a manifest entry would duplicate the source and would
  still need a flag to switch between systems.
- **Variants only (E19.8), no second independent system.** Rejected: the first
  case of E6.21 is two systems that share interfaces and some components but not
  a production base; writing one as a variant of the other states a relation
  that does not exist.
- **Infer the system from the package or the unit `ridl build` runs in.**
  Rejected: a unit may hold several systems (decision 1), so the inference would
  fail in the case this record enables.
- **One instance lock per system.** Rejected by decision 6.

## Consequences

- A workspace with two systems that list one shared component checks, builds
  each system with `--system` (or `--deployment`), and diffs both, which is the
  done criterion of E6.21.
- A workspace with one system is unchanged in source, output and command line.
- The book keeps exactly one `system` fence, now as a book convention stated in
  `CONTRIBUTING.md` rather than a consequence of RSDL-601 (question 3).
- `WorkspaceOutput` changes shape, which affects `ridl-lsp`, `ridl-mcp` and
  every caller that reads `system`.

## Questions resolved

The record was proposed with these five questions. The maintainer answered each
on 2026-11-15, and the answers are folded into the decisions.

1. **Does `ridl build` require `--system` with several systems?** The issue
   proposed that it does. Resolved 2026-11-15 by the maintainer: no; a build
   with neither flag writes every system, and `--system` or `--deployment`
   narrows it (decision 3).
2. **Does `ridl diff` require `--system` with several systems, as the issue
   says?** Resolved 2026-11-15 by the maintainer: no; every system is compared
   by name, and `--system` restricts the system part (decision 5). A diff is a
   report, and requiring a flag would make a plain `ridl diff` fail on a valid
   workspace.
3. **Does the book keep exactly one `system` fence?** ADR-0022 decision 10 rests
   on RSDL-601. Resolved 2026-11-15 by the maintainer: yes, as a book convention
   stated in `CONTRIBUTING.md`, since the book is one workspace.
4. **Two systems whose artifact names differ only in case.** Resolved 2026-11-15
   by the maintainer: a new error, RSDL-605, case-blind like RSDL-711, reported
   on the later system and blocking that system only (decision 4).
5. **Order of `systems`.** Resolved 2026-11-15 by the maintainer: by qualified
   name, so the output does not depend on the order files are read (decision 4).

## Documents amended

These documents would change with the acceptance.

| Document                                                                                       | Change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`docs/specification/rsdl-language-reference.md`](../specification/rsdl-language-reference.md) | §1.5: retitled; any number of systems, each with its own closure (decision 1). §2: the "One system per workspace" bullet removed. §3.1: closure checks per system; the RSDL-601 sentence removed. §3.4: `for` names one of the systems (decision 2). §7: one lock over the union of closures; RSDL-314 (decision 6). §13: lowering per system, closure errors block their own system (decision 4). §14: comparison by name, system added or removed (decision 5). §16 table: RSDL-601 moved to §16.3; RSDL-314 wording; RSDL-605 added (decision 4) |
| [ADR-0022](ADR-0022-rsdl-system-in-the-ir.md)                                                  | decision 2: one artifact per system. decision 8: a closure error blocks its own system only. decision 9: comparison by system name, the snapshot side unchanged. decision 10 (lines 223 to 225): the fence rule no longer rests on RSDL-601. Status: a paragraph for the amendment                                                                                                                                                                                                                                                                  |
| [`docs/technotes/rsdl-implementation.md`](../technotes/rsdl-implementation.md)                 | lines 186 to 189, the paragraph that derives one book fence from RSDL-601; `WorkspaceOutput::systems`                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| [`docs/design/codegen-plugins.md`](../design/codegen-plugins.md)                               | "The selection": the deployment is chosen within the selected system; `--system`                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| [`docs/design/deployment-descriptor.md`](../design/deployment-descriptor.md)                   | a descriptor is emitted for the deployments of the selected system                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| [`docs/book/rsdl.md`](../book/rsdl.md)                                                         | "The system": "declares at most one" replaced by the selection rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| [`docs/book/cli-reference.md`](../book/cli-reference.md)                                       | `--system` on `ridl build`, `ridlc build` and `ridl diff`; the exit-2 cases                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| [ADR-0029](ADR-0029-stability-policy.md)                                                       | an amendment that allows the removal of RSDL-601 from the `diagnostics` surface under decision 4, as for RIDL-150                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| [`CONTRIBUTING.md`](../../CONTRIBUTING.md)                                                     | "Writing examples in the book": the exactly-one `system` fence rule no longer rests on RSDL-601                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| [`AGENTS.md`](../../AGENTS.md)                                                                 | the conventions bullet on book fences: the one `system` fence and RSDL-601                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| [`docs/ROADMAP.md`](../ROADMAP.md)                                                             | E19.2: "the workspace's one system" becomes the union of the systems' closures                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| [ADR-0010](ADR-0010-cli-conventions.md)                                                        | the `build` and `diff` rows of the exit-code table: a missing, unknown or ambiguous `--system` is exit 2                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| [`freeze/README.md`](../../freeze/README.md) and the dumps under `freeze/`                     | the `cli` and `diagnostics` dumps regenerated with the implementation (decision 6); `accepted-sources` only if a frozen copy with two systems is added                                                                                                                                                                                                                                                                                                                                                                                              |
| [`docs/specification/ridl-family-overview.md`](../specification/ridl-family-overview.md)       | §5: a decision-ledger row for this record                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| [`docs/decisions/README.md`](README.md)                                                        | the entry for this record loses its _Proposed_ mark                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

## References

- driftsys/ridl#1397 — the tracking issue for E6.21.
- [ADR-0002](ADR-0002-module-system.md) — units, packages and the workspace.
- [ADR-0010](ADR-0010-cli-conventions.md) — CLI conventions and exit codes.
- [ADR-0022](ADR-0022-rsdl-system-in-the-ir.md) — the rsdl system in the IR.
- [ADR-0029](ADR-0029-stability-policy.md) — the stability policy and the frozen
  surfaces.
- `docs/ROADMAP.md` — E6.21, E19.2 (the instance lock) and E19.8 (variants).
