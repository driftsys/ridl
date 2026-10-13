# ridl

ridl is the interaction layer: the contracts a system offers and consumes. Every
interaction references typl vocabulary for its payload; ridl never defines a
shape inline.

## Interaction kinds — state or occurrence, action or request

See [Signal](https://driftsys.github.io/ridl/reference/ridl.html#4-signal),
[Event](https://driftsys.github.io/ridl/reference/ridl.html#5-event),
[Command](https://driftsys.github.io/ridl/reference/ridl.html#6-command),
[Query](https://driftsys.github.io/ridl/reference/ridl.html#7-query) and
[Fixed](https://driftsys.github.io/ridl/reference/ridl.html#8-fixed).

The choice of kind is a judgement, not a style preference — each kind makes a
different promise, and the wrong one misleads every consumer.

| Kind      | Carries             | Promise                                                           | Who initiates      | Can fail functionally?                     |
| --------- | ------------------- | ----------------------------------------------------------------- | ------------------ | ------------------------------------------ |
| `signal`  | state               | the latest sample is the truth; an intermediate one may be missed | provider publishes | no — the payload may itself be error-typed |
| `event`   | an occurrence       | every occurrence matters; occurrences are queued, never coalesced | provider publishes | no                                         |
| `command` | an action           | fire-and-forget; no reply                                         | consumer calls     | no — it has no return                      |
| `query`   | a request           | a reply is mandatory                                              | consumer calls     | yes — a fallible query returns `T \| E`    |
| `fixed`   | a provisioned value | held for the running software instance's lifetime, cacheable      | neither            | no                                         |

**State versus occurrence is the one distinction to get right.** If an "event"
would carry the whole current state at the moment it fires, rather than the fact
that something happened, it is a `signal`. A door that is open or closed is
state — `signal doorOpen: boolean`. The moment the door opened is an occurrence
— `event doorOpened: DoorPayload`.

**Action versus request is the other.** `command` has no reply, by construction
(`RIDL-104`, an explicit return type on a command is an error). An action whose
caller needs to know the outcome is a `query`, not a command with an imagined
return value. A command's observable effect comes back as state — the CQRS
discipline: commands mutate, queries and signals report.

```ridl
package skill.ridl.kinds

import skill.typl.evolution.GearPosition

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// A door's position index.
enum DoorIndex {
  FRONT_LEFT = 0
  FRONT_RIGHT = 1
}

/// Raised when a door's open state changes.
struct DoorPayload {
  door: DoorIndex
  isOpen: boolean
}

/// A vehicle identification number.
type Vin: string [17 match VIN_PATTERN] = "1HGCM82633A004352"

/// The VIN pattern.
const VIN_PATTERN = /^[A-HJ-NPR-Z0-9]{17}$/

/// An averaging window, in milliseconds.
type Window: integer [0..100000]

/// Cruise control and door state.
interface VehicleStatus {
  /// Continuous state: the latest sample is the truth.
  signal currentSpeed: Speed @10ms

  /// A discrete occurrence: every door event matters.
  event doorOpened: DoorPayload @[50ms..500ms]

  /// Fire-and-forget: no reply, so no return type.
  command setGear(position: GearPosition) @[..50ms]

  /// Request/response: a reply is mandatory.
  query getAverageSpeed(window: Window): Speed @[..200ms]

  /// Provisioned once, held for the software instance's lifetime.
  fixed vin: Vin
}
```

## Timing

See
[Timing Annotations](https://driftsys.github.io/ridl/reference/ridl.html#9-timing-annotations).

The `@` annotation is part of the contract. `@Xms` is strict periodic, valid on
a `signal` only (`RIDL-103` on anything else — an isochronous rate is
meaningless for an occurrence or a call). `@[min..max]` is the range form, valid
on every timed kind; either bound may be omitted. The two bounds have one
meaning, and what a binding does at each bound is derived from the declaring
keyword:

| Bound                   | `signal`                                   | `event`                               | `command` / `query`                              |
| ----------------------- | ------------------------------------------ | ------------------------------------- | ------------------------------------------------ |
| `min` — rate floor      | debounce: a faster update is coalesced     | throttle: no faster than `min`        | call throttle: the caller must not call faster   |
| `max` — staleness bound | refresh ceiling: re-publish even unchanged | TTL: an older occurrence is discarded | response bound: the provider must respond within |

An untimed `signal` or `event` takes the configurable default `@[100ms..1000ms]`
and draws `RIDL-100`; an untimed `command` or `query` takes `command_timing`
(`[..1s]`) or `query_timing` (`[..3s]`) and draws `RIDL-112`. `fixed` takes no
annotation at all (`RIDL-106`).

```ridl
package skill.ridl.timing

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// Raised when a fault is detected.
struct FaultPayload {
  code: integer [0..65535]
}

/// Timing on every kind that accepts it.
interface Timed {
  /// Strict periodic -- signal only.
  signal currentSpeed: Speed @10ms

  /// Change-driven, with both bounds.
  event fault: FaultPayload @[100ms..2000ms]

  /// A response bound, with no call throttle.
  command resetFaults() @[..50ms]

  /// A call throttle and a response bound.
  query getFaultLog(): FaultPayload @[20ms..200ms]
}
```

## Contracts — `require` and `ensure`

See
[Contracts — require / ensure](https://driftsys.github.io/ridl/reference/ridl.html#13-contracts--require--ensure).

`require` is a precondition over parameters and the interface's own signals,
valid on `command` and `query`; `ensure` is a postcondition over `result`, valid
on `query` only (`ensure` on a `command` is `RIDL-302`; either on a `signal`,
`event` or `fixed` is `RIDL-301`, because none of those has a call site). A
violated `require` is reported to the caller as a contract error, not as the
query's own error arm (the `RIDL-303` result union is for failures the domain
expects, never for a malformed call).

```ridl
package skill.ridl.contracts

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// The maximum speed a target may be set to.
const MAX_SPEED: Speed = 250.0

/// An averaging window, in milliseconds.
type Window: integer [0..100000]

/// Whether cruise control is engaged.
type Engaged: boolean

/// Cruise control's speed target.
interface CruiseControl {
  /// The current cruise state, read by `require` below.
  signal engaged: Engaged @[100ms..1s]

  /// Rejects a target above the limit, or while disengaged.
  command setTarget(target: Speed) @[..50ms] [
    require engaged
    require target <= MAX_SPEED
  ]

  /// Replies with a non-negative average.
  query getAverageSpeed(window: Window): Speed @[..200ms] [
    require window > 0
    ensure result >= 0.0
  ]
}
```

## Errors as data on a query

See
[Stratum 1 — Functional Errors Are Data](https://driftsys.github.io/ridl/reference/ridl.html#101-stratum-1--functional-errors-are-data).

ridl has no `throws` and no exceptions. A query that can fail as part of its own
domain says so in its return type: the inline `T | E` form, where the left arm
is any non-error named type and the right arm is exactly one error type (typl's
`error enum`, `error struct` or `error union`). A query with no success arm is
`RIDL-303`. A `command` carries no functional-error channel at all, by
construction — it has no return, so there is nowhere to put one; an outcome the
caller must know travels back as state or through a `query`.

```ridl
package skill.ridl.errors

import skill.typl.errors.CalReport
import skill.typl.errors.CalError

/// An axle index.
type Axle: integer [0..3]

/// A fallible calibration query.
interface Calibration {
  /// The inline `T | E` form -- the canonical spelling of a fallible query.
  query calibrate(axle: Axle): CalReport | CalError @[..200ms]
}
```

## Services

See
[Interfaces and Services](https://driftsys.github.io/ridl/reference/ridl.html#14-interfaces-and-services)
and
[Service — the global published declaration](https://driftsys.github.io/ridl/reference/ridl.html#145-service--the-global-published-declaration).

An `interface` is an abstract, reusable contract shape with no identity and no
address. A `service` gives one or more interfaces a global dotted name,
addressable as `service.member`; it is what an rsdl `component` `offers` and
what deployment realizes. A service may instead declare one inline shape, for a
one-off contract not worth a reusable interface — never both forms at once.
Composing several interfaces in one service is legal only while no member name
repeats across them (`RIDL-144`); a service name is unique across the workspace
(`RIDL-140`).

```ridl
package skill.ridl.services

/// A fan level.
type FanLevel: integer [0..7]

/// Cabin temperature.
type Temperature: integer [-40..85]

/// The climate interface, realized by the service below.
interface Climate {
  signal cabinTemperature: Temperature @[100ms..1s]
  command setFan(level: FanLevel) @[..50ms]
}

/// The climate interface's one service.
service skill.ridl.services.climate: Climate

/// A one-off diagnostic service, with an inline shape instead of a named
/// interface.
service skill.ridl.services.diag {
  query readTemperature(): Temperature @[..100ms]
}
```

## Where to split an interface

See
[Interface — the abstract shape](https://driftsys.github.io/ridl/reference/ridl.html#140-interface--the-abstract-shape)
and the lint table of
[Lints](https://driftsys.github.io/ridl/lints.html#the-lints).

There is no interface inheritance, so an interface boundary is a cohesion
judgement, not a syntax check. Call the `ridl_metrics` MCP tool and read
`RIDL-414` (`low-cohesion-interface`, an info-level design lint) as evidence: it
groups an interface's members by the named types they share — the payload of a
`signal`, `event` or `fixed`, and the parameters, return type and error type of
a `command` or `query` — and reports the interface when its members fall into at
least seven disconnected groups. An interface at that size is usually several
contracts bundled under one name; splitting it along the reported groups keeps
each importer's grant to the narrowest interface it needs — a `requires` line in
rsdl names an interface, never a service, and the grant it reaches is the
interface's.

```ridl,allow=RIDL-414
package skill.ridl.cohesion

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// Cabin temperature.
type CabinTemperature: integer [-40..85]

/// A door's position index.
enum DoorZone {
  FRONT = 0
  REAR = 1
}

/// A fan level.
type FanLevel: integer [0..7]

/// A fault code.
type FaultCode: integer [0..65535]

/// A seat position.
type SeatPosition: integer [0..100]

/// A headlight mode.
enum HeadlightMode {
  OFF = 0
  ON = 1
}

/// An interface whose members share no type with each other -- bundled
/// under one name for no reason the shapes themselves give.
interface Bundle {
  signal currentSpeed: Speed @10ms
  signal cabinTemperature: CabinTemperature @[1s..10s]
  event doorOpened: DoorZone @[50ms..500ms]
  command setFan(level: FanLevel) @[..50ms]
  query getFaultCode(): FaultCode @[..200ms]
  command setSeatPosition(position: SeatPosition) @[..50ms]
  signal headlight: HeadlightMode @[1s..10s]
}
```

## Evolution

See
[Interaction Identity and Evolution](https://driftsys.github.io/ridl/reference/ridl.html#11-interaction-identity-and-evolution).

An interaction's identity is its ordinal, assigned by declaration order across
every interaction of the interface regardless of kind, so evolution is
append-only: add a new interaction at the end, and retire a removed one with a
`reserved` tombstone at the ordinal it held, rather than deleting it or
inserting in its place. Before editing a published interface, call `ridl_diff`
between the old and the new source and require exit 0 (compatible or identical)
unless the break is deliberate and stated as such.

```ridl
package skill.ridl.evolution

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// Raised when a fault is detected.
struct FaultPayload {
  code: integer [0..65535]
}

/// An interface, after one interaction was retired and one added.
interface VehicleStatus {
  signal currentSpeed: Speed @10ms
  reserved legacyTemp // was ordinal 2 -- retired, never reused
  event fault: FaultPayload @[100ms..2000ms]
}
```

## Common mistakes

See
[Diagnostics](https://driftsys.github.io/ridl/reference/ridl.html#16-diagnostics).

| Mistake                                                    | Correct form                                                    | Code       |
| ---------------------------------------------------------- | --------------------------------------------------------------- | ---------- |
| giving a `command` a return type                           | use a `query`, or observe the outcome as state                  | `RIDL-104` |
| a `query` returning `()`                                   | use a `command`                                                 | `RIDL-105` |
| strict periodic `@Xms` on an `event`, `command` or `query` | use the range form `@[min..max]`                                | `RIDL-103` |
| `require`/`ensure` on a `signal`, `event` or `fixed`       | validity is the typl constraint; there is no call site to guard | `RIDL-301` |
| `ensure` on a `command`                                    | a command has no `result` to constrain                          | `RIDL-302` |
| a fallible query with no success arm                       | an inline `T \| E` whose left arm is a non-error type           | `RIDL-303` |
| two interfaces in one `service` that share a member name   | rename one of them before composing                             | `RIDL-144` |
| redeclaring an interaction under a `reserved` name         | pick a new name; the retired one stays retired                  | `RIDL-401` |
| an interface with members in many disconnected groups      | split along the groups `ridl_metrics` reports                   | `RIDL-414` |
