---
name: ridl-authoring
description: >-
  Write, review and evolve RIDL-family contracts: typl types, ridl interfaces
  and rsdl systems in a workspace with a ridl.toml. Use when creating or
  editing .typl, .ridl or .rsdl files or a ridl.toml, when turning a
  requirement into an interface or a type, or when reviewing such a change.
metadata:
  version: "0.13.0"
---

## What RIDL is

See [The family](https://driftsys.github.io/ridl/introduction.html#the-family).
RIDL is a family of languages that share one toolchain, `ridl`:

- `typl` describes data: types, ranges, units and constants.
- `ridl` describes the interactions at a boundary: signals, events, commands and
  queries.
- `rsdl` describes the architecture: components, wiring and deployment.

A project is a workspace: a `ridl.toml` and its `.typl`, `.ridl` and `.rsdl`
files. `rmdl` (behaviour) is specified only.

## Check your work

See
[the CLI reference](https://driftsys.github.io/ridl/cli-reference.html#ridl-mcp).
Never hand over a source that `ridl` has not checked.

1. After every edit, check the workspace and fix every diagnostic, in order.
   With the `ridl` MCP server connected, call `ridl_check`, and `ridl_explain`
   with a code for its rule. Without it, run `ridl check --format json` and read
   the code in the Diagnostics reference below; never guess a rule.
2. When you change an existing workspace, copy it outside the workspace before
   the first edit. Compare the copy with the edited workspace with `ridl_diff`
   (or `ridl diff <old> <new>`). A copy taken after the edit reports no change.
   Exit 0 is required unless a break is deliberate and stated.
3. End with the result of the check, and of the diff when you ran one.

If `ridl` is not installed, see
[Installing the toolchain](https://driftsys.github.io/ridl/getting-started.html#installing-the-toolchain)
and ask the user before you install anything.

## A first look

A ranged unit type, a constant and an enum, each documented (more in
[Your first interface](https://driftsys.github.io/ridl/getting-started.html#your-first-interface)):

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

A minimal component and system:

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

Read a reference before using a new construct; `ridl_explain` a code, never
guess.

- [typl — types and vocabulary](https://driftsys.github.io/ridl/reference/typl.html#4-primitives)
- [ridl — system interactions](https://driftsys.github.io/ridl/reference/ridl.html#4-signal)
- [rsdl — architecture](https://driftsys.github.io/ridl/reference/rsdl.html#32-component)
- [Diagnostics](https://driftsys.github.io/ridl/reference/typl.html#16-diagnostics)
