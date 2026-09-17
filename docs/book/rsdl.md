# Describing a system

[Getting started](getting-started.md) declares contracts: the types a system
carries and the interactions it offers. This chapter declares what uses them —
which components exist, which contracts each one offers and requires, how the
copies are bundled, and which machine each copy runs on. That is rsdl, the
architecture layer of the family, and the [rsdl reference](reference/rsdl.md) is
its specification.

**What is built.** The compiler reads `.rsdl` files, checks every rule of the
reference, and lowers the checked system to the IR beside the package IR.
`ridl diff` compares two systems.

**What is not built.** No runtime reads the lowered system. The links, the
routing table and the grants this chapter shows are facts the compiler writes
down; nothing delivers a message along them. There is no descriptor emitter and
no transport.

## Contracts first

rsdl declares no type, no interface and no service. An `.rsdl` file that tries
draws RSDL-604. The contracts live in a ridl package, exactly as the getting
started chapter writes them:

```ridl
package veh.climate

type Temperature : integer [-40..85]
type FanLevel : integer [0..7]
type Heated : boolean

interface Climate {
  signal cabinTemperature : Temperature @[100ms..1s]
  command setFan(level : FanLevel) @[..50ms]
}

interface Seats {
  signal heated : Heated @[100ms..1s]
}

service veh.climate.control : Climate
service veh.climate.seats : Seats

service veh.climate.diag {
  query readTemperature() : Temperature @[..100ms]
}
```

`veh.climate.diag` declares its contract inline rather than naming a reusable
interface, so its dotted name is also the name of its one interface.

## Components

A component is one execution context. It offers services, requires interfaces,
and names one reference per line:

```rsdl,allow=RSDL-409
package veh.cabin

import veh.climate.Climate
import veh.climate.Seats

/// Runs the climate loop. Two copies, one per zone controller.
component ClimateControl [ instances = (front, rear) ] {
  offers   veh.climate.control
  requires Seats
}

component SeatHeating { offers veh.climate.seats }

component Dashboard {
  requires Climate
  requires Seats
}

/// The phone app. No implementation in this workspace.
component PhoneApp [ external ] {
  requires Climate
  requires veh.climate.diag
}
```

- **`requires` names an interface, never a provider.** The compiler finds the
  one service in the closure that lists that interface, and the one component
  that offers that service. A consumer that requires the narrowest interface it
  needs is unaffected when the service gains or splits interfaces.
- **`instances` names the copies.** A component that declares none has one
  instance, written `Unit` in the IR and in diagnostics.
- **`external` marks a component with no implementation in this workspace.** It
  is still part of the closure, and its links are still derived.
- **`ClimateControl` has two instances, so it is a redundant provider set.**
  `Dashboard`'s and `PhoneApp`'s `requires Climate` each draw RSDL-409, "not yet
  realizable": every link is lowered to each instance, and there is no
  arbitration. The fence above allows RSDL-409 for that reason.

## The system

A `system` is the closure — the components the build is about. A workspace
declares at most one:

```rsdl
package veh.cabin

system Cabin { ClimateControl, SeatHeating, Dashboard, PhoneApp, veh.climate.diag }
```

`veh.climate.diag` is a lone service that no declared component offers, so it
stands for an implicit component of that name, with one `Unit` instance. Writing
a `component` for it would say nothing the service does not already say.

## Distributions

A distribution is what is versioned, signed and shipped together. When a
workspace declares one, every implemented closure component is in exactly one:

```rsdl
package veh.cabin

distribution Comfort [ tier = PLATFORM ] { ClimateControl, SeatHeating, veh.climate.diag }
distribution Display [ tier = APPLICATION ] { Dashboard }
```

`PhoneApp` is external, so it is in no distribution. `Display` depends on
`Comfort`, because `Dashboard` requires interfaces that `ClimateControl` and
`SeatHeating` offer. The dependency is derived, never written. An application
distribution may depend on a platform one; the reverse is a tier inversion,
RSDL-901.

## Deployments and machines

A deployment places the closure on machines. Each machine lists what it hosts,
and every instance sits on exactly one machine per deployment:

```rsdl,allow=RSDL-804
package veh.cabin

deployment Car for Cabin {
  machine ZoneFront [ labels = (ASIL_A) ] { ClimateControl.front, SeatHeating, veh.climate.diag }
  machine ZoneRear { ClimateControl.rear }
  machine Head { Dashboard [ linux.cpuset = (1) ] }
  machine Cloud [ external ] { PhoneApp }
}

deployment Bench for Cabin {
  machine Rig { ClimateControl, SeatHeating, Dashboard, veh.climate.diag, PhoneApp }
}
```

- **A bare component name places every instance of it**, as `Rig` does with
  `ClimateControl`. `ClimateControl.front` places one.
- **An `external` machine hosts external components only** (RSDL-707). On `Rig`,
  `PhoneApp` is placed on an on-board machine: a stub standing in for the phone.
- **`linux.cpuset` is a backend key**, carried uninterpreted on `Dashboard`'s
  placement. No configured backend claims the namespace `linux`, so it draws
  RSDL-804, which the fence allows.

Two mistakes are worth naming, because each blocks only its own deployment.
Placing `ClimateControl.front` on `ZoneFront` and then reaching
`ClimateControl.rear` twice in `Car` is RSDL-706, an instance placed twice.
Dropping the `ZoneRear` machine leaves `ClimateControl.rear` on no machine,
which is RSDL-701. Either one leaves the other deployment's lowering intact.

## What the compiler writes

`ridl check` reports every rule above. `ridl build` writes the package IR and
the lowered system beside it:

```sh
ridl build --emit ir-json --out-dir out && ls out
```

```text
veh.cabin.Cabin.system.json
veh.cabin.ir.json
veh.climate.ir.json
```

The system artifact is named after the system's qualified name. `--emit ir-text`
and `--emit ir-binary` write `.system.txtpb` and `.system.binpb` instead.

The facts inside it are stated once for the closure and once per deployment.
For the closure:

- **The components**, with their instances, their `offers` and `requires` lines,
  and their `external` flag.
- **The producers** — for each service, the component that offers it and that
  component's instances. `veh.climate.control` is marked not yet realizable,
  because `ClimateControl` has more than one instance.
- **The grants** — per component, the catalog regions its requirements reach.
  `PhoneApp` reaches `veh.climate`, and its grant is lowered with the `external`
  flag: it is derived here, and enforcing it is the external side's business.
  `SeatHeating` requires nothing, so its grant is empty.
- **The region map** — one region per catalog the closure reaches, here
  `veh.climate`, holding `Climate`, `Seats` and `veh.climate.diag` with the
  number each takes from its package's `interfaces.lock`.
- **The distributions**, with the derived dependency.

Per deployment:

- **The placements** — every instance with the machine it runs on, including an
  instance with no link at all.
- **The links**, one per consumer instance and producer instance, each with its
  crossing kind. In `Car`, `ClimateControl.front → SeatHeating.Unit` is same
  machine, `ClimateControl.rear → SeatHeating.Unit` is different machine, and
  every link from `PhoneApp` is off-board. In `Bench` every link is same
  machine, because everything sits on `Rig`.
- **The routing table** — the key `(catalog, interface number, member ordinal)`
  to the producing instances and their machines, for every member of every
  interface a closure service lists.
- **The surface set** — the system's external boundary. Here it is the three
  links from `PhoneApp`, with the external side consuming, in both deployments:
  the `external` flag and not the machine defines the boundary, so `Bench` keeps
  them although they cross nothing.
- **The installation** of each distribution — in `Car`, `Comfort` on `ZoneFront`
  and `ZoneRear`, `Display` on `Head`.

**What an error does.** An error in the closure blocks the lowering of every
deployment, and the build writes nothing. An RSDL-7xx error — a placement
problem — blocks its own deployment only: the build writes the package IR and
the system without that deployment, and still exits 1.

## Comparing two systems

`ridl diff` compares the contracts by the ridl categories, and those alone give
the verdict and the exit code. The changes that are not contract changes are
listed after them, under two headings and with no verdict. Moving `Dashboard`
from `Head` to `ZoneRear` in `Car`:

```text
identical
placement changed
  Car/veh.cabin.Dashboard.Unit: Head -> ZoneRear
```

The contracts did not move, so the verdict is `identical` and the exit code 0.
A `requires` line added, a component added to the system, `instances` changed or
a component made `external` are listed under `composition changed` instead.
Which of these changes break an already deployed system, and for whom, is not
something the compiler decides.

## What is not here yet

- **No runtime reads the lowered system,** and no descriptor is emitted from it.
  The facts are written down and go no further.
- **rsdl declares no transport, no network and no process.** A same-machine
  crossing says two instances share a machine, not how they talk. If a backend
  needs such a fact, it arrives as that backend's namespaced key.
- **Posture, redundancy arbitration and the `optional` requirement are
  reserved** — see [§12 of the reference](reference/rsdl.md). A redundant
  provider set is reported, not resolved.
