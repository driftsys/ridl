---
schema: 1
name: ridl-authoring
description: >-
  Write, review and evolve RIDL-family contracts: typl types, ridl interfaces
  and rsdl systems in a workspace with a ridl.toml. Use when creating or
  editing .typl, .ridl or .rsdl files or a ridl.toml, when turning a
  requirement into an interface or a type, or when reviewing such a change.
metadata:
  version: "0.10.0"
---

## The model in one minute

One platform, one grammar, one IR, four languages, each describing one concern:

- `typl` — the vocabulary: types, ranges, units, constants.
- `ridl` — interactions at every boundary of a system, a person, or the world.
- `rmdl` — behaviour, as functions and reactive models. Specified only; it has
  no implementation in this toolchain.
- `rsdl` — architecture: components, wiring, deployment.

The dependency lattice runs one way, `typl ← {ridl, rmdl} ← rsdl`: a file never
references a vocabulary declared only downstream of it.

```typl
package skill.intro

/// Cabin air temperature, in degrees Celsius.
type Temperature : Cel [-40.0..85.0 step 0.5]
```

## The verify loop

Never trust a generated source unchecked.

1. Write the `.typl`, `.ridl` or `.rsdl` source.
2. Call the `ridl_check` MCP tool. Fix every diagnostic it reports, in the order
   it reports them; call `ridl_explain` with a diagnostic's code for its
   severity and a one-line summary when the report alone is not enough. For the
   full rule behind a code, read the language reference named by
   [Which reference to read](#which-reference-to-read) below.
3. Before editing an interface or a type that is already published, call
   `ridl_diff` between the old and the new source. Exit 0 (compatible or
   identical) is required unless a breaking change is deliberate and stated as
   such.
4. Ground new work in what the workspace already declares, rather than inventing
   a name: call `ridl_list_interactions`, `ridl_describe_type` and
   `ridl_resolve` before writing an `import` or a reference. For a boundary or a
   naming decision that needs judgement rather than syntax, call `ridl_metrics`
   and read the design lints (`RIDL-414`, `TYPL-222`, `TYPL-224`) it reports as
   evidence.

## Which reference to read

- `rules.md` — the always-on constraints: read it first, every time.
- `typl.md` — writing or changing vocabulary: types, ranges, units, constants,
  composites, errors as data, packages, evolution.
- `ridl.md` — writing or changing an interface: the interaction kinds, timing,
  contracts, services, evolution.
- `rsdl.md` — writing or changing a system: components, wiring, deployment.
