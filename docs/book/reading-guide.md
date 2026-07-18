# Reading guide

The specifications are separate by design — five audiences, five learnable
wholes. Pick the path for your role; each links to the full reference in the
repository. The family overview
([`docs/specification/ridl-family-overview.md`](https://github.com/driftsys/ridl/blob/main/docs/specification/ridl-family-overview.md))
is the map that ties them together.

- **Data architect** — [typl](https://github.com/driftsys/ridl/blob/main/docs/specification/typl-language-reference.md),
  end to end. It stands alone.
- **Service / bus SSOT engineer** — typl §1–§10, then
  [ridl](https://github.com/driftsys/ridl/blob/main/docs/specification/ridl-language-reference.md)
  end to end.
- **UX / frontend engineer** — typl §1–§10, ridl's core semantics, then
  [uxdl](https://github.com/driftsys/ridl/blob/main/docs/specification/uxdl-language-reference.md).
- **Control / algorithm engineer** — typl §1–§10, ridl §3/§9, then
  [rmdl](https://github.com/driftsys/ridl/blob/main/docs/specification/rmdl-language-reference.md).
- **Integrator / architect** — everything above at survey depth, then
  [rsdl](https://github.com/driftsys/ridl/blob/main/docs/specification/rsdl-language-reference.md).

For the motivation and the big picture, read the concept note
([`docs/wip/ridl-family-concept.md`](https://github.com/driftsys/ridl/blob/main/docs/wip/ridl-family-concept.md));
for the shared surface rules across every profile, the general form
([`docs/wip/family-general-form.md`](https://github.com/driftsys/ridl/blob/main/docs/wip/family-general-form.md)).
Building the toolchain? Start with
[the roadmap](https://github.com/driftsys/ridl/blob/main/docs/ROADMAP.md) and
the [decision records](https://github.com/driftsys/ridl/tree/main/docs/decisions).
