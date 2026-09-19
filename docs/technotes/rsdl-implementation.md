# rsdl as built: the checks, the lowering and the diff

How rsdl is implemented in this workspace, after lane B of the 2026-09-13 step-1
coordination (driftsys/ridl#328) landed stories E6.12 to E6.16, E6.18 and E6.19.
Informative: the normative records are
[the rsdl language reference](../specification/rsdl-language-reference.md) for
the language, and [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) for
the IR carrier, the build gate and the diff. The plan the implementation
followed, with its task sequence, is
[`docs/archive/2026-09-15-rsdl-plan.md`](../archive/2026-09-15-rsdl-plan.md).

## One workspace-level query, not a per-package pass

`.rsdl` is a third profile of the one family grammar: the parser gains the five
declarations, and the checks are a single salsa query,
`ridl_sem::check_system(db, ws, std)`, over every `.rsdl` file of the workspace.

It is workspace-level because the closure is (rsdl §3.1): a component declared
in one package offers a service declared in another and is placed by a
`deployment` declared in a third, so no per-package pass can see a closure. The
package checks (`check_package`) are unchanged by rsdl, and the two queries do
not depend on each other; the rsdl query reads the package resolver only to bind
a reference in a member line.

The query returns `CheckedSystem`, the collected model with its diagnostics.
Every entry in the model carries the source site it is written at, which is what
lets the language server report, hover and navigate over the same model the
compiler checks, with no second parse.

Two things in the model are **not** in any source file:

- **the implicit component** (rsdl §6) — a lone service that no `component`
  declares stands for a component of its own, and it exists as a model entry
  with the service's dotted name as its identity;
- **the unit instance** (rsdl §7) — a component that declares no `instances` has
  exactly one, named `Unit` (`ridl_sem::rsdl::UNIT_INSTANCE`). `Unit` is never
  written in source; a written `Unit` is kept in the model only so that its
  diagnostic, RSDL-307, can report it.

Both are model entries rather than synthesized source, so a diagnostic about
either reports against the line that implies it.

## RSDL-804 is raised by the drivers

`check_system` cannot raise RSDL-804 — a backend key whose namespace no
configured backend claims — because the query does not know which backends are
configured. The check is a separate function,
`ridl_sem::unclaimed_backend_keys(db, system, claimed, sources)`, which takes
the claimed namespaces as a parameter, and each driver calls it: `ridlc`, the
language server, and the corpus runner.

Today every one of them passes the empty set, because no backend declares the
keys it consumes until the plugin contract of
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
exists. So every backend key in an rsdl file draws the warning, and the key is
carried into the IR either way — the warning never blocks (rsdl §13).

## The lowering is a plain function over the checked model

`ridl_sem::lower_system(system: &CheckedSystem, packages: &[&v2::Package]) ->
Option<v2::System>`
is a plain function, not a salsa query. Only `ridlc` and `ridl diff` lower, once
per command run and after every query has returned; the language server never
lowers, so nothing would reuse a memoized result.

It reads the lowered package IR as its second argument, because rsdl §13's
inputs from outside rsdl live there: each interface's number and provisional
flag (`Interface.number`, `Interface.provisional`, from the package's lock) and
each member's ordinal (`Decl.ordinal`, ridl §11). An interface is found by
identity through `Package::shapes()`, the walk that sees an inline shape;
`Package.interfaces` alone misses one.

**Gating** is the function's own, and it is rsdl §13's:

- an error in the closure blocks every deployment, so the function returns
  `None` when `CheckedSystem::closure_has_errors` is set;
- an RSDL-7xx error blocks its own deployment only, which
  `DeploymentPlacement::has_errors` records, as does a deployment whose closure
  was never placed with no RSDL-7xx error raised for it: that deployment is left
  out of `System.deployments`, and the IR carries no marker for it — the
  diagnostic is the record;
- a warning never blocks;
- a workspace that declares no `system` lowers nothing, which is also `None`.

**Orderings are fixed** so that a reordering of the source does not reorder the
IR: placements are listed in closure order then instance order, links in
`requires` order then consumer-instance then producer-instance order, routes by
their key, regions by catalog name and their interfaces by interface number,
installation machines in declaration order, and dependencies by qualified name.

The crossing kind of a link is computed in the lowering, not in the placement
pass: same machine when the two placements name one machine, otherwise off-board
when either machine is `external`, otherwise different machine (rsdl §10). The
surface set holds the link itself with its direction, read from which endpoint's
component is `external` — the flag, not the machine, defines the boundary.

## What the pipeline writes

`ridlc::compile_workspace` lowers the system and returns it on
`WorkspaceOutput::system`, so a consumer of a compiled workspace — `ridl diff` —
reads the facts rather than lowering again.

`ridl build` writes the lowered system beside the package IR for each IR dump
emit, under the suffixes `Emit::system_dump_suffix` names
([ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 2). The
build gate is `blocks_every_artifact`: an error-severity diagnostic whose code
does not start with `RSDL-7` stops every write, as before; an RSDL-7xx error
alone lets every artifact be written and still exits 1
([ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 8).

The corpus entry `rsdl-appendix-a` carries the reviewed snapshot of a lowered
system, `corpus__system@rsdl-appendix-a.snap`; it is the only entry that gets
one, because it is the only clean entry that declares a `system`.

## `ridl diff` at the system

`ridl_diff::system` compares two lowered systems and returns `SystemChange`
values under one of two headings, `PlacementChanged` and `CompositionChanged`.
They are their own vocabulary rather than `Category` variants because a
`Category` carries a verdict and these carry none, which is also why `--explain`
does not list them.

The contract comparison is unchanged and still decides the verdict and the exit
code. The system changes are rendered after the contract lines — in text, the
heading on its own line and one indented `path: before -> after` line per change
with no `[verdict]`; in JSON, the optional keys `placement_changed` and
`composition_changed`, left out when empty — so a report with no system change
renders byte for byte as it did before.

Both sides must be source trees. A `.ir.json` snapshot carries no system, so
`ridl diff .ridl/baseline .` lists no system change rather than reporting the
whole system as added.

## rsdl in the book

The book-example harness compiles `rsdl` fences. It stages the whole book as one
workspace and names each staged file by its language word, so an `rsdl` fence
sits beside the `ridl` fences of every chapter and can name the services and
interfaces they declare. Because a workspace declares at most one `system`
(RSDL-601), the book holds exactly one `system` fence and every other `rsdl`
fence declares components, distributions or deployments, or is marked
`rsdl,ignore`. The harness adds no rule of its own here — a second `system` is
simply a diagnostic the fence did not allow. `CONTRIBUTING.md` and `AGENTS.md`
state the rule.

The chapter is [`docs/book/rsdl.md`](../book/rsdl.md), written as built: it says
there is no runtime, and every fact it states about the lowered system and about
`ridl diff` is pinned by a test.

## What is not built yet

- **The catalog hash per region** (story E6.17). rsdl §13 lists the catalog
  hashes of every catalog in the region map, as received. The function that
  computes one, `ridl_descriptor::hash::catalog_hash` (driftsys/ridl#324), does
  not exist, so `Region` has no hash field and the lowered system carries no
  hash. The archived plan's Part B4 Task 9 describes the work that fills it, and
  [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 7 records
  the constraint it must satisfy: the driver embeds the hash, the rsdl lowering
  never computes it.
- **A runtime's system descriptor.** rsdl §13 says a runtime's descriptor is an
  emitter over these facts, specified with the runtime. The runtime descriptors
  design defines two artifacts and the roadmap plans only the catalog half (Epic
  16); the per-deployment system descriptor takes its own rows now that Epic 6
  has landed, and has none yet. This workspace has no runtime, so nothing reads
  the lowered system at run time.
- **Backend namespace claims.** See RSDL-804 above.
