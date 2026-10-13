# rsdl

rsdl is the architecture layer: which components exist, which contracts each one
offers and requires, and which machine each copy runs on. It declares no type,
no interface and no service of its own — those are ridl and typl, in the same
package (`RSDL-604` catches a declaration of another profile in an `.rsdl`
file).

## Components — `offers` and `requires`

See
[`component`](https://driftsys.github.io/ridl/reference/rsdl.html#32-component).

A `component` is one execution context. It `offers` services and `requires`
interfaces; the verbs differ because the sides differ — a service is the unit of
publication and addressing, an interface the unit of contract, so half a service
cannot be offered. One reference per line: `requires A, requires B`, never
`requires A, B`. A `requires` line names an interface, never a provider — the
compiler finds the one component that offers the service listing it, so a
consumer is unaffected when that service gains or splits interfaces elsewhere.

```ridl
package skill.rsdl.cabin

/// Cabin air temperature.
type Temperature: integer [-40..85]

/// Whether a seat heater is on.
type Heated: boolean

/// A fan level.
type FanLevel: integer [0..7]

/// The climate control contract.
interface Climate {
  /// The current cabin temperature.
  signal cabinTemperature: Temperature @[100ms..1s]
  /// Sets the fan level.
  command setFan(level: FanLevel) @[..50ms]
}

/// The seat heating contract.
interface Seats {
  /// Whether the seat heater is on.
  signal heated: Heated @[100ms..1s]
}

/// The climate control's one service.
service skill.rsdl.cabin.control: Climate

/// The seat heating's one service.
service skill.rsdl.cabin.seats: Seats
```

```rsdl
package skill.rsdl.cabin

/// Runs the climate loop. Requires the seat contract, so the dashboard is
/// not the only consumer of it.
component ClimateControl {
  offers skill.rsdl.cabin.control
  requires Seats
}

/// Runs the seat heaters.
component SeatHeating {
  offers skill.rsdl.cabin.seats
}

/// Shows both readings. Requires the narrowest interface it needs from
/// each service.
component Dashboard {
  requires Climate
  requires Seats
}
```

A component with no `instances` key has exactly one instance, written `Unit` in
diagnostics and in the IR; `Unit` itself is never written in source
(`RSDL-307`). Declaring copies that offer the same services names them:
`component ClimateControl [ instances = (front, rear) ] { ... }`. Two parts that
offer different services are two component declarations, never two instances of
one. `external` marks a component with no implementation in this workspace; it
is still part of the closure and its links are still derived.

## Systems

See [`system`](https://driftsys.github.io/ridl/reference/rsdl.html#31-system).

A `system` is the closure: the named set of components a deployment has to
account for, and nothing else. A workspace declares at most one; a second is
`RSDL-601`. A component the closure does not list is not lowered, with no
diagnostic — leaving one out is silent, so list every component the build is
about.

```rsdl
package skill.rsdl.cabin

/// The cabin system: every component the build accounts for.
system Cabin {
  ClimateControl
  SeatHeating
  Dashboard
}
```

A service that no declared component offers stands for an
[implicit component](https://driftsys.github.io/ridl/reference/rsdl.html#6-the-implicit-component)
of the service's own dotted name, with its unit instance and nothing else — a
system of hand-written services, with no component written for any of them, is
legal and is a list of service names.

## Deployments and machines

See
[`deployment`](https://driftsys.github.io/ridl/reference/rsdl.html#34-deployment),
[`machine`](https://driftsys.github.io/ridl/reference/rsdl.html#35-machine) and
[Placement](https://driftsys.github.io/ridl/reference/rsdl.html#9-placement).

A deployment places the closure on machines, `for` a declared system. Every
instance of every closure component sits on exactly one machine per deployment —
one left unplaced is `RSDL-701`, one reached twice is `RSDL-706`. A bare
component name places every instance of it; `Name.instance` places one. An
`external` machine hosts `external` components only (`RSDL-707`). A system takes
several deployments — a production topology and a bench topology differ only
here.

```rsdl
package skill.rsdl.cabin

/// The bench deployment: one machine hosting every component.
deployment Bench for Cabin {
  machine Rig {
    ClimateControl
    SeatHeating
    Dashboard
  }
}
```

Two machines on one deployment give every link between them a crossing kind —
`same machine`, `different machine`, or `off-board` when one side is `external`
— derived from where each link's two instances are placed, never declared.

## Evolving a system compatibly

See
[`ridl diff` at the System](https://driftsys.github.io/ridl/reference/rsdl.html#14-ridl-diff-at-the-system).

`ridl diff` compares at the system: the closure's services, the interfaces they
list and their members, classified by the ridl categories — it classifies
contracts only. Two kinds of change it reports carry no verdict, because they
change no contract:

- **Placement changed** — an instance moved to another machine, a machine added,
  removed, or made `external`, or a changed `labels`, `deprecated` or backend
  key on a deployment, a machine, or a placement line.
- **Composition changed** — a component added to or removed from the system, an
  `offers` or `requires` line added or removed, `instances` changed, a component
  made `external`, or a changed `labels`, `deprecated`, `tier` or backend key on
  the system, a component, a distribution, or one of their lines.

A changed `depth`, `slots` or `budget` is not listed at all: a sizing value
changes no link and no contract. Before changing a published system, call
`ridl_diff` between the old and the new source; a breaking contract change — an
interface member changed in a breaking direction — is still reported even when
the closure that reaches it did not change.

```rsdl,ignore
// A fragment: adding a redundant copy of a provider changes composition,
// not contract. Every `requires` resolving to it then draws RSDL-409 --
// not yet realizable, because arbitration between the copies is reserved.
component ClimateControl [ instances = (front, rear) ] {
  offers skill.rsdl.cabin.control
  requires Seats
}
```

## Common mistakes

See
[Diagnostics](https://driftsys.github.io/ridl/reference/rsdl.html#16-diagnostics).

| Mistake                                                                 | Correct form                                                     | Code       |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------- | ---------- |
| `requires A, B` on one line                                             | one reference per line: `requires A, requires B`                 | `FORM-102` |
| a component requiring an interface listed by a service it itself offers | require it from the component that actually needs the other side | `RSDL-308` |
| the same service on two `offers` lines of one component                 | list it once                                                     | `RSDL-309` |
| `offers` naming something that is not a service                         | name a declared `service`                                        | `RSDL-310` |
| `requires` naming a multi-interface service                             | name one of its interfaces, not the service                      | `RSDL-311` |
| two closure components offering one service                             | offer it from one component; add instances for redundancy        | `RSDL-502` |
| a second `system` in the workspace                                      | declare one system per workspace; remove the extra declaration   | `RSDL-601` |
| an instance placed on no machine in a deployment                        | list it on exactly one machine                                   | `RSDL-701` |
| an instance reached twice in one deployment                             | list it on exactly one machine, once                             | `RSDL-706` |
| an `external` machine listing an implemented component                  | place the implemented component on an on-board machine           | `RSDL-707` |
