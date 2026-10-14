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

See [The family](https://driftsys.github.io/ridl/introduction.html#the-family).
`typl` is vocabulary (types, ranges, units, constants), `ridl` interactions at a
boundary, `rsdl` architecture (components, wiring, deployment); `rmdl`
(behaviour) is specified only. The lattice: `typl ← {ridl, rmdl} ← rsdl`.

## The tooling

One `ridl` binary serves three surfaces
([CLI reference](https://driftsys.github.io/ridl/cli-reference.html#ridl-mcp)).

- **MCP** (`ridl mcp`): read-only tools an agent calls: `ridl_check`,
  `ridl_explain`, `ridl_diff`, `ridl_metrics`, `ridl_list_interactions`,
  `ridl_describe_type`, `ridl_resolve`, `ridl_references`, `ridl_dependencies`.
  Prefer them; they teach no syntax, the examples and references below do.
- **CLI**: when no MCP is connected: `ridl check --format json`,
  `ridl diff <old> <new>`, `ridl fmt`, `ridl new` to scaffold, `ridl help`.
- **LSP** (`ridl lsp`): editor diagnostics; an agent does not call it.

## Installing

See
[Installing the toolchain](https://driftsys.github.io/ridl/getting-started.html#installing-the-toolchain).
If `ridl` is not on the `PATH`, tell the user, and run an installer only after
they agree:

- Linux and macOS:
  `curl -fsSL https://raw.githubusercontent.com/driftsys/ridl/main/install.sh | bash`
- Windows:
  `irm https://raw.githubusercontent.com/driftsys/ridl/main/install.ps1 | iex`
- VS Code: the "RIDL" extension bundles `ridl`, runs the language server and
  registers `ridl mcp` for Copilot (VS Code 1.109 or newer); the command "RIDL:
  Install ridl to PATH" puts the binary on the `PATH`.
- Another agent host: register `ridl mcp` as a stdio server, for Claude Code
  `claude mcp add ridl -- ridl mcp`
  ([agent setup](https://driftsys.github.io/ridl/getting-started.html#editor-and-agent-setup)).

## The verify loop

Never trust a generated source unchecked; always report the loop's result.

1. Write the source, then call `ridl_check`; fix every diagnostic in order. Call
   `ridl_explain` with a code for its severity and rule. Treat `TYPL-406`
   (`missing-docs`) like any diagnostic — fix it, never ignore it.
2. Before the first edit of an _existing_ workspace, copy it outside first — a
   copy taken after editing makes `ridl_diff` report `identical` no matter what
   changed. Diff the outside copy (`old`) against the edited workspace (`new`);
   exit 0 is required unless a break is deliberate and stated.
3. Ground new work in what the workspace declares: call
   `ridl_list_interactions`, `ridl_describe_type` and `ridl_resolve` before an
   `import`; use `ridl_metrics` for a naming judgement (`RIDL-414`, `TYPL-222`).
4. When a requirement has no schema expression (retry, ordering, storage), say
   so, name what the schema does and does not enforce, and describe the rest in
   prose.
5. In the final message, state the `ridl_check` result and any `ridl_diff`
   verdict.
6. Document every exported declaration and member with what a caller must know,
   not the range, unit or timing it already states (see
   [House style](https://driftsys.github.io/ridl/documenting.html#house-style)).

## Basic examples

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
