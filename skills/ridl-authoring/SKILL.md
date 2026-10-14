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

See [The family](https://driftsys.github.io/ridl/introduction.html#the-family):
one platform, one grammar, one IR, four languages, one concern each.

- `typl` — vocabulary: types, ranges, units, constants.
- `ridl` — interactions at every boundary: system, person, world.
- `rmdl` — behaviour. Specified only; no implementation in this toolchain.
- `rsdl` — architecture: components, wiring, deployment.

The lattice runs one way, `typl ← {ridl, rmdl} ← rsdl`.

```typl
package skill.intro

/// Cabin air temperature, in degrees Celsius.
type Temperature : Cel [-40.0..85.0 step 0.5]
```

## The verify loop

See [`ridl mcp`](https://driftsys.github.io/ridl/cli-reference.html#ridl-mcp).
Never trust a generated source unchecked; always report the loop's result.

1. Write the source, then call `ridl_check`; fix every diagnostic in order. Call
   `ridl_explain` with a code for its severity and rule. Treat `TYPL-406`
   (`missing-docs`) like any diagnostic — fix it, never ignore it.
2. Before the first edit of an _existing_ workspace, copy it outside first — a
   copy taken after editing makes `ridl_diff` report `identical` no matter what
   changed. Diff the outside copy (`old`) against the edited workspace (`new`);
   exit 0 is required unless a break is deliberate and stated.
3. Ground new work in what the workspace declares — call
   `ridl_list_interactions`, `ridl_describe_type` and `ridl_resolve` before an
   `import`, and `ridl_metrics` for a naming judgement (`RIDL-414`, `TYPL-222`,
   `TYPL-224`).
4. When a requirement has no schema expression — retry, ordering, storage,
   enforcement beyond `@` — say so; name what the schema does and does not
   enforce, and prose the rest.
5. In the final message, state the `ridl_check` result and, for an edit to a
   published contract, the `ridl_diff` verdict as printed.

## Documenting types and interfaces

See [House style](https://driftsys.github.io/ridl/documenting.html#house-style).
Document what a caller must know — what a value means, where it comes from, what
it does when it changes — not the range, unit or timing the declaration already
states. Every exported declaration, and every member of one, takes a doc comment
unless its parent is `internal`, as the types example below shows.

## Basic examples

A ranged unit type, a constant, a struct and an enum, each member documented
(see
[Your first interface](https://driftsys.github.io/ridl/getting-started.html#your-first-interface)
for the shapes; verify each block with `ridl_check` before relying on it):

```typl
package skill.intro.types

/// Road speed.
type Speed : km/h [0.0..250.0 step 0.5]
/// The regulatory top of the speed range.
const MAX_SPEED : Speed = 250.0

/// The selected drive mode.
enum DriveMode {
  /// No assist.
  NORMAL = 0
  /// Economy assist.
  ECO = 1
}

/// The state of cruise control.
struct CruiseControlState {
  /// Whether cruise control is engaged.
  enabled : boolean
  /// The requested speed.
  targetSpeed : Speed
}
```

A signal, an event, a `require`-contracted command, and a fallible query:

```ridl
package skill.intro.cruise

type Speed : km/h [0.0..250.0 step 0.5]
const MAX_SPEED : Speed = 250.0
type Engaged : boolean
type FaultCode : integer [0..65535]
error enum CalError { SENSOR_UNAVAILABLE = 0 }
struct CalReport { offset : float [-1.0..1.0 step 0.001] }

/// Cruise control: engagement, faults, speed target, calibration.
interface CruiseControl {
  signal engaged : Engaged @[100ms..1s]
  event fault : FaultCode @[50ms..500ms]
  command setTarget(target : Speed) @[..50ms] [
    require engaged
    require target <= MAX_SPEED
  ]
  query calibrate() : CalReport | CalError @[..200ms]
}
```

A minimal component and system — the smallest rsdl closure a build accounts for:

```ridl
package skill.intro.cabin

type Temperature : integer [-40..85]

interface Climate {
  signal cabinTemperature : Temperature @[100ms..1s]
}

service skill.intro.cabin.control : Climate
```

```rsdl
package skill.intro.cabin

component ClimateControl { offers skill.intro.cabin.control }

system Cabin { ClimateControl }
```

## Where to find the docs

Read a reference first; `ridl_explain` a code instead of guessing.

- [typl — types and vocabulary](https://driftsys.github.io/ridl/reference/typl.html#4-primitives)
- [ridl — system interactions](https://driftsys.github.io/ridl/reference/ridl.html#4-signal)
- [rsdl — architecture](https://driftsys.github.io/ridl/reference/rsdl.html#32-component)
- [Diagnostics (typl's section; ridl, rsdl carry the same one)](https://driftsys.github.io/ridl/reference/typl.html#16-diagnostics)
- [Lints — the `[lints]` table](https://driftsys.github.io/ridl/lints.html#the-lints-table)
- [Documenting your API](https://driftsys.github.io/ridl/documenting.html#a-documented-package)
- [Getting started](https://driftsys.github.io/ridl/getting-started.html#what-ridl-is)
