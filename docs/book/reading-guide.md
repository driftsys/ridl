# Reading guide

The specifications are separate by design — one learnable whole per audience.
Each path below is a route through the same four languages, not a different set
of them. Pick the path for your role; each links to the full reference. The
family overview
([`docs/specification/ridl-family-overview.md`](https://github.com/driftsys/ridl/blob/main/docs/specification/ridl-family-overview.md))
is the map that ties them together.

Only typl and ridl have a toolchain. A path below that ends in rxdl, rmdl or
rsdl ends in a specification you can design against but cannot compile — see
[what is built](introduction.md#what-is-built).

- **Data architect** — [typl](reference/typl.md), end to end. It stands
  alone.
- **Service / bus SSOT engineer** — typl §1–§10, then [ridl](reference/ridl.md)
  end to end.
- **UX / frontend engineer** — typl §1–§10, ridl's core semantics and the
  presentation and intent families, then [rxdl](reference/rxdl.md) (specified,
  not built).
- **Sensor / actuator engineer** — typl §1–§10, ridl's acquisition and control
  families and their correspondence obligations, then
  [rxdl](reference/rxdl.md) §5 (specified, not built).
- **Control / algorithm engineer** — typl §1–§10, ridl §3/§9, then
  [rmdl](reference/rmdl.md) (specified, not built).
- **Integrator / architect** — everything above at survey depth, then
  [rsdl](reference/rsdl.md) (specified, not built).
- **Auditor / safety assessor** — the family overview's shared doctrines, then
  typl's keyword registry and evolution model, ridl's error strata and services,
  and the diagnostics table of each reference.

Writing your first `.ridl` file rather than reading a specification? Start with
[Getting started](getting-started.md).

For the motivation and the big picture, read the concept note
([`docs/wip/ridl-family-concept.md`](https://github.com/driftsys/ridl/blob/main/docs/wip/ridl-family-concept.md));
for the shared surface rules across every profile, the general form
([`docs/wip/family-general-form.md`](https://github.com/driftsys/ridl/blob/main/docs/wip/family-general-form.md)).
Building the toolchain? Start with
[the roadmap](https://github.com/driftsys/ridl/blob/main/docs/ROADMAP.md) and
the [decision records](https://github.com/driftsys/ridl/tree/main/docs/decisions).
