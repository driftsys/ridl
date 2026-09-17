# ADR-0022 — The rsdl system in the IR: its artifact, the build gate, and `ridl diff` at the system

## Status

Accepted — 2026-09-18. Scope: the decisions the rsdl implementation needed that
the rsdl language reference does not settle — where the lowered system lives,
what artifact carries it, which facts of rsdl §13 the IR states and which it
does not, how `ridl build` gates on an error that blocks one deployment only,
and how `ridl diff` reports a change that carries no verdict. It binds the IR
every later consumer reads — including the system descriptor of the runtime
descriptors design, which is not yet planned — the `ridl build` contract, and
`ridl diff`.

Written from lane B of the 2026-09-13 step-1 coordination (driftsys/ridl#328),
which built rsdl's parser, checks, lowering and `ridl diff` support (stories
E6.12 to E6.16, E6.18 and E6.19). The reasoning trail and the task-by-task
sequence are
[`docs/archive/2026-09-15-rsdl-plan.md`](../archive/2026-09-15-rsdl-plan.md);
the implementation as built is described in
[the rsdl implementation technote](../technotes/rsdl-implementation.md).

Sebastien delegated the plan's decisions P-B1 to P-B9 to the lane driver on
2026-09-15 and confirmed decision 8 below on 2026-09-17.

It does not restate the rsdl language reference: §13 lists the facts the
lowering produces and §14 states the two headings and that they carry no
verdict. This record fixes the engineering choices those sections leave open.
[ADR-0014](ADR-0014-ir-encodings.md) decision 4 carries the artifact naming in
its 2026-09-18 amendment; decision 2 below states why it is written by the IR
dump emits and nothing more.

## Context

rsdl §13 states the lowering as facts and leaves their carrier open: "a
runtime's descriptor is an emitter over them and is specified with the runtime".
The toolchain still needed one carrier now, because three consumers already
exist — `ridl build`, `ridl diff` at the system, and the corpus snapshots — and
because the runtime descriptors design reads the facts rather than the source.

Two properties of the system made the choice harder than "add fields to the
package IR". A system is workspace-wide: one `system` per workspace (rsdl §1.5),
over packages that are each independently published, so it is not a package
fact. And its errors are not all equal: rsdl §13 makes an error in the closure
block every deployment, while an RSDL-7xx error blocks its own deployment only —
a gate `ridl build` did not have, because its rule was that any error writes
nothing.

## Decision

1. **The system facts live in the IR, in a `System` message of their own** (plan
   decision P-B1). `crates/ridl-ir/proto/ridl/ir/v2/system.proto`, package
   `ridl.ir.v2`, compiled by the same `protox` pass as `ir.proto`, with `System`
   as the root message. It is its own artifact beside the package IR, not a
   field of `Package`: a package is published on its own and a system is
   workspace-wide. This follows roadmap epic E6's exit criteria ("the IR carries
   …"); a runtime's system descriptor is a later emitter over this message, not
   a second lowering from source.

2. **The artifact is `<pkg.Name>.system.{json,txtpb,binpb}`, written by the
   three IR dump emits.** No new `--emit` value: `ir-json`, `ir-text` and
   `ir-binary` each write the system in their own encoding when the workspace
   declares one, through a second suffix table (`Emit::system_dump_suffix`)
   beside `Emit::ir_dump_suffix`, with the same wildcard-free `match`. The two
   tables stay separate because the `.system.` infix is load-bearing: `ridl`'s
   snapshot surface reads every `.ir.json` file as a package and `ridl baseline`
   publishes only those, so the infix is what keeps a system out of a baseline.
   The name cannot collide with a package artifact — a package name is lowercase
   and a system's qualified name ends in a CamelCase segment.
   [ADR-0014](ADR-0014-ir-encodings.md) decision 4's 2026-09-18 amendment
   records the naming on the encodings record.

3. **§13's facts and nothing else, with the closure stated once.** The closure
   facts are stated once and the per-deployment facts once per deployment that
   no RSDL-7xx error blocked. §13 lists the producers with the machine of each
   instance; a machine is a placement fact, so a `Producer` names the component
   and its instances and a reader takes the machines from
   `Deployment.placements`, which lists every instance. Nothing is lost and no
   fact is repeated. `tier` and `deprecated` are **not** lowered: they are
   rsdl-owned keys consumed into checks (rsdl §5, RSDL-901), and §13 lists
   neither. The model carries no doc comment for an rsdl declaration, so the
   message has no `doc` field.

4. **Identity in the message is by qualified name, with `inline` beside it.** A
   component is `pkg.Name`, or the owning service's dotted name for the implicit
   component (rsdl §6); the case of the last segment tells the two apart. A
   machine name is bare, scoped to its deployment (rsdl §3.5). Because the
   inline shape of `service cabin` and an `interface cabin` may be spelled the
   same in one package, `InterfaceRef` and `RegionInterface` carry an `inline`
   flag beside the name, which is the same distinction the lock draws with its
   `service:` prefix. A body line of a `system` or a `distribution` is a
   `MemberLine`, not a `Member`: in ridl a member is one typed interaction of an
   interface.

5. **The routing key is (catalog, interface number, member ordinal), and a
   `reserved` tombstone has no route.** ridl §11 counts a tombstone in the
   ordinal sequence and it holds its ordinal, but it is no member, and rsdl §13
   routes "every member of every interface" — so a tombstone produces no route
   and the member after it keeps its ordinal. A `fixed` member is a member and
   has a route. The routing table is per deployment, because its values are the
   producing instances with their machines, and routes are sorted by the key.
   The number and the provisional flag are read from the lowered package IR
   (`Interface.number`, `Interface.provisional`), never recomputed by rsdl.

6. **The region of an interface is the catalog of the package that declares
   it,** not the package of the owning service (rsdl §11). A region lists its
   interfaces in interface-number order and the regions are in catalog-name
   order. A grant is listed for **every** closure component, with an empty
   region list for a component that requires nothing, so the permission list is
   total over the closure rather than over the consumers.

7. **The region carries no catalog hash yet.** rsdl §13 makes the hash an input,
   computed by ridl over a catalog and embedded here; the function that computes
   it (`ridl_descriptor::hash::catalog_hash`, driftsys/ridl#324) does not exist
   yet, so `Region` has **no hash field** and story E6.17 stays open. When it
   lands, the driver embeds the hash — `ridl_sem::rsdl::lower_system` must not
   depend on `ridl-descriptor` — and `Region` gains `bytes hash = 3`. The
   archived plan's Part B4 Task 9 is the record of that deferred work.

8. **`ridl build` writes every artifact when the only errors are RSDL-7xx, and
   still exits 1** (plan decision P-B8, confirmed by Sebastien on 2026-09-17).
   The build's rule was that any error-severity diagnostic writes nothing. rsdl
   §13 narrows it: an RSDL-7xx error blocks the lowering of its own deployment
   only, so every package artifact and the system without that deployment are
   sound and are written. The exit code is unchanged — an error was reported, so
   the run exits 1, and [ADR-0010](ADR-0010-cli-conventions.md)'s `build` row
   still holds. Every other error — a closure error, a typl, ridl or MANI error
   — still writes nothing. `ridl baseline` keeps refusing on any error, so an
   RSDL-7xx error never publishes a baseline.

9. **`ridl diff` compares every package's contracts, then the system, and the
   system changes carry no verdict** (plan decisions P-B5 and P-B9). The
   contract comparison is unchanged: every package of each side, by the ridl
   categories, and they alone decide the verdict and the exit code. Restricting
   it to the closure's reachable contracts would hide a breaking change to a
   published package this workspace does not use. The rsdl changes are listed
   after them under the two rsdl §14 headings — "placement changed" and
   "composition changed" — as a `SystemHeading` in `ridl_diff::system`, beside
   `Category` and never inside it, because a `Category` carries a verdict and
   these do not; `--explain` does not list them. Exactly the §14 changes are
   listed: a change to `labels`, to a backend key or to a distribution is not.
   **Both sides must be source trees:** only a compiled workspace carries a
   lowered system, an `.ir.json` snapshot carries none, so
   `ridl diff .ridl/baseline .` lists no system change rather than reporting the
   whole system as added.

10. **The book holds exactly one `system` fence** (plan decision P-B3). The
    book-example harness stages the whole book as one workspace and now compiles
    `rsdl` fences, so an `rsdl` block names services declared in `ridl` blocks
    in any chapter. A workspace declares at most one `system` (RSDL-601), so a
    second `system` fence anywhere in the book fails the harness like any
    unallowed diagnostic — the harness adds no rule of its own. `AGENTS.md` and
    `CONTRIBUTING.md` state the rule for contributors.

## Consequences

- The IR has two root messages and two artifact families. A tool that reads "the
  IR" must say which: `Package` for contracts, `System` for who produces them,
  where each copy runs and how they link.
- A build whose only errors are RSDL-7xx now leaves artifacts on disk with a
  non-zero exit. Anything that treats exit 1 as "nothing was written" must be
  read again; `ridl baseline` already refuses on any error and is unaffected.
- `ridl diff`'s JSON output gains two optional keys, `placement_changed` and
  `composition_changed`, each left out when empty, so a report without a system
  change renders byte for byte as before.
- The IR is one field short of rsdl §13 until story E6.17 lands: no consumer can
  yet check a catalog hash from the system artifact.

## Open

1. **Story E6.17, the catalog hash per region** (decision 7), which waits on
   driftsys/ridl#324's `ridl_descriptor::hash::catalog_hash`.
2. **Which system changes are breaking, and for whom** — the stability policy's
   question (roadmap E4.5a), which is why decision 9 reports them with no
   verdict.
3. **Which backend claims which namespace** — RSDL-804 warns on every backend
   key until a backend can declare the keys it consumes, which waits for the
   plugin contract of
   [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md).
4. **A baseline that carries a system.** Decision 9 lists no system change when
   either side is a snapshot; a baseline that publishes a `.system.json`
   artifact would change that, and nothing yet requires it.

## Documents amended

| Document                                                | Change                                                                                                                                                                                        |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [ADR-0014](ADR-0014-ir-encodings.md) decision 4         | a 2026-09-18 amendment names the system artifact and its three suffixes, and records that the `.system.` infix keeps it out of `.ir.json` snapshot detection (landed with the implementation) |
| [ADR-0010](ADR-0010-cli-conventions.md) the `build` row | unchanged by decision 8 — the exit code is still 1 on any error; what is written on an RSDL-7xx error is stated here, since that row never stated what is written                             |

## References

- [`docs/archive/2026-09-15-rsdl-plan.md`](../archive/2026-09-15-rsdl-plan.md) —
  the implementation plan, with decisions P-B1 to P-B9 and the per-task
  decisions this record carries forward; read it as a plan, not as a description
  of the result
- [the rsdl implementation technote](../technotes/rsdl-implementation.md) — the
  checks, the lowering and the diff as built
- [rsdl language reference](../specification/rsdl-language-reference.md) — §13
  the facts, §14 the two headings, §11 grants and regions, §16 the diagnostics
- [ADR-0014](ADR-0014-ir-encodings.md) — the three encodings and the artifact
  names
- `crates/ridl-ir/proto/ridl/ir/v2/system.proto`,
  `crates/ridl-sem/src/rsdl/lower.rs`, `crates/ridl-diff/src/system.rs` — the
  message, the lowering and the diff as built
