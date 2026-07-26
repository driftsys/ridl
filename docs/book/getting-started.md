# Getting started

A practical introduction to **ridl**, the interface description language of the
RIDL family. It is written for engineers who describe the contracts between
software components — in a vehicle, or in any component-based reactive system.

Every `ridl` code block in this chapter is compiled by the repository's test
suite against the toolchain in this repository, and must draw no diagnostic
beyond the ones its own fence names. If a block is shown here, it compiles.
The prose around the blocks is not machine-checked; where it describes
behaviour rather than syntax, read
[the note below](#what-the-contract-requires-means-in-this-chapter) first.

## What ridl is

ridl answers one question about a service: _what does it produce, consume, and
guarantee?_

A `.ridl` file describes an **interface** — not an implementation. It does not
say how a service works internally, only what it exposes at its boundary. That
is what makes it usable as the single source of truth between teams,
components, and tools.

ridl is transport-neutral by design: the same contract is intended to bind to
SOME/IP, gRPC, DDS, MQTT, or AIDL without modification. The transport bindings
are specified in the ridl language reference, Appendix B. **They are not
implemented yet.** What the compiler emits today is listed under
[What the compiler produces](#what-the-compiler-produces).

### What "the contract requires" means in this chapter

Several sections below describe what happens when a contract runs: that a
subscriber is delivered the last value, that an occurrence past its staleness
bound is discarded, that a provider checks a `require` clause before your code
sees the call. **Every one of those is a requirement the specification places on
a future runtime and its transport bindings. None of it is implemented.** There
is no runtime in this repository — eleven compiler crates, and nothing that
delivers a message.

The sentences are written in the present tense because they describe what the
contract means, which is the thing you are designing against. They do not
describe behaviour you can observe today. What you can observe today is the
toolchain: it checks your declarations, generates data types, samples your
`require` clauses, and classifies changes between versions. Where a comment
inside a listing says "discarded after 2 s", read it as what the contract
demands of a binding, not as something running.

### ridl is one language of five

ridl does not define types. It references them. The vocabulary layer is a
separate language, **typl**, and the two are used together: a `.ridl` file
accepts every typl declaration in addition to its own interaction
declarations.

| Language | Describes                                                                | Status in this repository |
| -------- | ------------------------------------------------------------------------ | ------------------------- |
| **typl** | data — types, ranges, units, constants                                   | built                     |
| **ridl** | system interactions — `signal`, `event`, `command`, `query`, `final`     | built                     |
| **uxdl** | user interactions — `display`, `input`, `action`, `fetch`, `fixed`       | specified, not built      |
| **rmdl** | behaviour — functions and reactive models                                | specified, not built      |
| **rsdl** | architecture — components, wiring, deployment                            | specified, not built      |

This chapter covers typl and ridl only, because those are the two you can run
today. Where it mentions the other three, it is describing a plan.

## Building the toolchain

There is no published release yet. Build the two binaries from a clone of this
repository:

```sh
cargo build --release
```

That produces `target/release/ridl` — the command you will use — and
`target/release/ridlc`, the compiler underneath it. Put `ridl` on your `PATH`,
or call it by path.

## A workspace

A **package is a directory**, and every package carries a `ridl.toml` manifest.
A workspace groups several packages under a root manifest that lists them.

The tutorial builds this layout:

```text
tutorial/
├── ridl.toml                 the workspace root
└── veh/
    ├── common/
    │   ├── ridl.toml
    │   ├── types.ridl        the shared vocabulary
    │   └── scalars.ridl
    ├── cluster/
    │   ├── ridl.toml
    │   └── cluster.ridl      the interfaces
    ├── identity/             annex 1
    ├── powertrain/           annex 2
    ├── body/                 annex 3
    └── dms/                  annex 4
```

The root manifest lists every member. Start with the two the tutorial builds
first, and add the annex packages as you reach them:

```toml
[workspace]
members = ["veh/common", "veh/cluster"]
```

Each member names itself and its version:

```toml
[package]
name = "veh.common"
version = "1.0.0"
```

The package name mirrors the directory path below the workspace root, and every
file in the directory declares that same name. A mismatch is a hard error
(TYPL-002).

## Your first vocabulary

Create `veh/common/types.ridl`:

```ridl
package veh.common

/// Road speed of the vehicle.
type Speed : km/h [0.0..250.0 step 0.5]
```

That is a complete package. `type Speed : km/h [0.0..250.0 step 0.5]` declares a
named scalar with a physical unit (UCUM `km/h`), a closed range, and a
quantization step.

## Your first interface

Create `veh/cluster/cluster.ridl`:

```ridl
package veh.cluster

import veh.common.Speed

/// Vehicle speed reporting interface.
interface VehicleSpeed {

  /// Current vehicle speed, published every 10 ms.
  signal currentSpeed : Speed @10ms
}
```

Line by line:

- `package veh.cluster` — this file belongs to the `veh.cluster` package.
- `import veh.common.Speed` — `Speed` is declared in another package. Imports
  are single-type and qualified; there are no wildcards and no relative
  imports.
- `interface VehicleSpeed` — the contract boundary.
- `signal currentSpeed : Speed @10ms` — a continuously published value, whose
  contract requires the provider to publish it every 10 ms.

## Checking it

From the workspace root:

```sh
ridl check
```

`ridl check` type-checks a file, a package directory, or a whole workspace, and
defaults to the current directory. It exits 0 when nothing is wrong, 1 when a
diagnostic is an error, and 2 when it cannot even load the workspace — for
example when no `ridl.toml` is found.

Swap the bounds in `types.ridl` to `[250.0..0.0 step 0.5]` and run it again:

```text
error[TYPL-104]: range minimum 250 is greater than maximum 0
  ┌─ ./veh/common/types.ridl:4:19
  │
4 │ type Speed : km/h [250.0..0.0 step 0.5]
  │                   ^^^^^^^^^^^^^^^^^^^^^
```

Most diagnostics carry a stable code. `TYPL-` codes come from the vocabulary
layer, `RIDL-` codes from the interaction layer, `FORM-` codes from the shared
surface syntax, and `MANI-` codes from the manifest. Those codes are listed in
the language references and are never renumbered or reused. A few diagnostics
are still uncoded and print as a bare `error:` — an unresolved type name is
one — so do not assume a code is always there to search for.

## Primitives and named types

typl has five primitives: `boolean`, `integer`, `float`, `string`, `bytes`.

**In an interaction, a primitive is never written directly.** A signal payload,
an event payload, a command or query parameter, and a `final` value must each
name a type. Writing `final doorCount : integer [1..8]` is a `FORM-102` error;
declare `type DoorCount : integer [1..8]` and use the name. This is the single
rule that most often surprises newcomers, and it is what gives every value on
the boundary a domain meaning rather than a width.

Struct fields are more permissive. An inline constrained primitive is accepted
there, including `string [1..10]` and `bytes [64]` — what the compiler rejects
in a field is a **bare** `string` or `bytes` with no constraint at all
(`TYPL-208`). The typl reference states the rule without that qualification, so
trust the compiler here: the constraint is what the check is about, and the
reference's own Appendix B writes `frame : bytes [8]`. Named types remain the
recommendation everywhere, and they are mandatory on a boundary.

Add a second file, `veh/common/scalars.ridl`:

```ridl
package veh.common

// physical unit types — UCUM units
type Temperature : Cel   [-40.0..125.0 step 0.1]
type FuelLevel   : %     [0.0..100.0 step 0.1]
type Voltage     : V     [0.0..48.0 step 0.1]
type Ratio       : %     [0.0..100.0 step 0.1]

// constrained integer types
type Counter   : integer [0..65535]
type DoorIndex : integer [0..7]
type DoorCount : integer [1..8]
type Severity  : integer [0..5]

// a named boolean, so it can be used on a boundary
type Enabled : boolean

// string types — bounds are recommended everywhere and required on a boundary
type ModelCode : string [3..6 match MODEL_PATTERN] = "ABC"
type Notes     : string [1..256] = "-"
```

Two details in that listing:

- The range drives the wire width. `integer [0..255]` is a `uint8` on
  transport; `integer [0..65535]` is a `uint16`. The generated language type is
  always `int64`. You never write a width.
- `= "ABC"` is an **init value** — the value a consumer holds before anything
  real arrives. It is derived automatically for most types: `false` for
  booleans, `0` for a number whose range contains it and the range minimum
  otherwise, and the empty value when a `string` or `bytes` bound allows length
  0. It cannot be derived for a `string` **or** `bytes` type carrying a `match`
  pattern or a non-zero minimum length, so those declare one — which is why
  `ModelCode` and `Notes` above do. Without a declared init such a type draws a
  `TYPL-115` note. That note stays informational, always; what turns a missing
  init into an error is _using_ the type as a signal payload, and the error is
  `RIDL-109`, raised at the signal.

Standard library types are always available without an import: `Uuid`, `Ulid`,
`Vin`, `Uri`, `Url`, `Email`, `Label`, `Name`, `Message`, `Timestamp`,
`Duration`, `Version`, `Date`, `CountryCode`, `Sha256Hash`, and others. They
live in the implicitly imported `ridl.std` package.

## Constants

`const` declares a compile-time constant, reusable in range bounds, `match`
patterns, init values, and contract expressions:

```ridl
package veh.common

const MAX_SPEED      : Speed   = 250.0
const SPEED_LIMIT_EU : Speed   = 130.0
const MAX_GEAR       : integer = 6

// regex constants
const MODEL_PATTERN = /^[A-Z][A-Z0-9]{2,5}$/

// reuse in a range bound
type EngineSpeed : km/h [0.0..MAX_SPEED step 0.5]

struct GearState {
  gear : integer [0..MAX_GEAR]
}
```

Constants are written `SCREAMING_SNAKE` by convention. The compiler does not
enforce it — the lexer folds every case shape into one identifier kind, so a
lower-case constant name compiles. The declared value, on the other hand, must
satisfy the constant's own type constraints, and that is checked.

## Signals and events

The split between `signal` and `event` is the load-bearing distinction in ridl.
It is a distinction in _meaning_, which the specification then turns into
demands on a binding (see the note above — no binding exists yet):

- A **signal** is continuous state. The latest sample is the truth, and an
  intermediate sample may be missed. The contract requires the channel never to
  be empty: a new subscriber must be delivered a value immediately — the init
  value before the first publication, the latest value after it.
- An **event** is a discrete occurrence. Every occurrence matters, so the
  contract requires occurrences to be queued rather than coalesced, and forbids
  late-joiner delivery. An occurrence that happened before you subscribed did
  not happen to you.

```ridl
package veh.cluster

import veh.common.Speed
import veh.common.Temperature
import veh.common.FuelLevel
import veh.common.SPEED_LIMIT_EU

/// The three timing forms a signal accepts.
interface Sampling {

  /// Strict periodic — published every 10 ms whether or not it changed.
  signal currentSpeed : Speed @10ms

  /// Change-driven — not faster than 20 ms, refreshed at least every 100 ms.
  signal engineTemp : Temperature @[20ms..100ms]

  /// Staleness bound only — refreshed at least every 5 s.
  signal fuelLevel : FuelLevel @[..5s]

  /// An init value override, written as a bare `= value` before the timing.
  signal targetSpeed : Speed = SPEED_LIMIT_EU @[20ms..500ms]
}
```

The bounds have one meaning everywhere: `min` is the **rate floor**, the
minimum interval between publications, and `max` is the **staleness bound**,
the maximum age of the value. Those two bounds are what the compiler records in
the IR, and they are all it records. What a binding is _required_ to do at each
bound follows from the declaring keyword — specified, not implemented:

| Bound                   | Required on a `signal` (state)                 | Required on an `event` (occurrence)       |
| ----------------------- | ---------------------------------------------- | ----------------------------------------- |
| `min` — rate floor      | debounce — coalesce a faster update            | throttle — do not raise occurrences faster |
| `max` — staleness bound | refresh ceiling — re-publish even if unchanged | TTL — discard an older occurrence          |

Events take a range only. Strict periodic `@Xms` on an event is a `RIDL-103`
error, because an isochronous rate is meaningless for occurrences:

```ridl
package veh.cluster

import veh.common.DoorIndex

struct DoorPayload {
  sensorId : DoorIndex
  isOpen   : boolean
}

interface DoorEvents {

  /// Raised on every door state change; a binding must treat it as stale
  /// 500 ms after it was raised.
  event doorOpened : DoorPayload @[50ms..500ms]

  /// A binding must throttle to one occurrence per 100 ms, and must discard
  /// one that reaches a consumer more than 2 s after it was raised.
  event speedLimitExceeded : DoorPayload @[100ms..2000ms]
}
```

A signal or event with no `@` annotation is not rejected — it receives the
default range `@[100ms..1000ms]` and draws a `RIDL-100` warning. The default is
configurable per package or per workspace:

```toml
[defaults]
timing = "[100ms..1000ms]"
```

Because the IR always carries resolved bounds, changing that default changes
every untimed interaction in the package, and `ridl diff` reports it as a
contract change. Safety-graded packages should annotate every interaction
explicitly.

## Enums and enum sets

An `enum` is an integer-backed set of discrete values, for choosing one:

```ridl
package veh.common

enum GearPosition {
  PARK    = 0
  REVERSE = 1
  NEUTRAL = 2
  DRIVE   = 3
  LOW     = 4
}

enum DriveMode { NORMAL = 0, ECO = 1, SPORT = 2, OFF_ROAD = 3 }
```

Newline and comma are interchangeable separators everywhere, so both layouts
above are the same declaration. There are no semicolons in the language.

An `enumset` is a named bitfield, for several flags at once:

```ridl
package veh.common

enumset AccessFlags {
  DOOR_UNLOCKED = 0    // bit 0
  BOOT_UNLOCKED = 1    // bit 1
  WINDOW_OPEN   = 2    // bit 2
}
```

When you need both the single value and the set, declare the `enum` and derive
the `enumset` from it:

```ridl
package veh.common

enum Warning {
  LOW_FUEL     = 0
  CHECK_ENGINE = 1
  DOOR_OPEN    = 2
  SEATBELT     = 3
}

enumset WarningFlags : Warning
```

Both names are then usable on a boundary — the enum where one value is meant,
the enumset where several are:

```ridl
package veh.cluster

import veh.common.Warning
import veh.common.WarningFlags

interface Warnings {
  command clearWarning(flag: Warning)
  signal  activeWarnings : WarningFlags @[50ms..1s]
}
```

## Provisioned values — final

A `final` is a value set at build, factory, or over-the-air update, and
immutable for the lifetime of the running software instance:

```ridl
package veh.cluster

import veh.common.DoorCount
import veh.common.Enabled

interface VehicleIdentityBasics {
  final vin             : Vin
  final softwareVersion : Version
  final marketRegion    : Label
  final ecuSerial       : Uuid
  final capabilities    : [Label; 0..32]
  final doorCount       : DoorCount
  final hasCruise       : Enabled
}
```

Declaring a value `final` is a promise that it never changes while the software
runs, which is what makes it safe for a consumer to cache unconditionally — a
promise to a future binding, like the rest of the delivery semantics. What the
compiler enforces today is the shape: a `final` takes no timing annotation and
no contract block, and both are `RIDL-106` errors. Note `hasCruise : Enabled`
rather than `hasCruise : boolean`: a boundary value names a type.

## Commands and queries

A **command** is a fire-and-forget action request. It never returns a value —
writing a return type is a `RIDL-104` error. If the caller needs to know the
outcome, the outcome is observed as state, or the interaction is a query.

A **query** is request/response. The reply is mandatory, and a query returning
`()` is a `RIDL-105` error.

```ridl
package veh.cluster

import veh.common.Speed
import veh.common.GearPosition

interface Control {
  command setTargetSpeed(speed: Speed)
  command enableCruiseControl()
  command resetFaults()
  command setGear(position: GearPosition)

  query getCurrentSpeed(): Speed
  query getSpeedHistory(window: Duration): (min: Speed, max: Speed, avg: Speed)
}
```

A query return may be a named type, a tuple of named fields, a stream, or an
inline `T | E` union that makes the query fallible. Parameters are named types
or streams — a tuple is not a parameter type; pass a struct.

## Structs and optionality

```ridl
package veh.common

struct CruiseControlState {
  enabled     : boolean
  targetSpeed : Speed
  mode        : DriveMode
  override    : Speed?     // optional — absent when no override is active
}
```

The `?` suffix means _may be absent_, which is not the same as present-and-null;
typl has no null.

Struct fields carry implicit ordinals by declaration order, so evolution is
append-only: add new fields at the end, and retire a removed field with a
`reserved` tombstone so its slot is never reused.

```ridl
package veh.common

struct DriverProfile {
  name  : Name
  reserved legacyChecksum      // was ordinal 2 — retired, never reused
  speed : Speed
}
```

Structs may not be recursive: an unbounded wire size would contradict the
bounded-size guarantee every typl composite carries.

## Errors are data

ridl has no `throws`, no exceptions, and no status codes. A query that can fail
as part of its domain semantics says so in its return type, using vocabulary
declared in typl:

```ridl
package veh.common

/// Failure vocabulary — the `error` modifier marks a shape as a failure shape.
error enum CalError {
  SENSOR_UNAVAILABLE = 0
  VEHICLE_MOVING     = 1
  OUT_OF_RANGE       = 2
}

struct CalReport {
  offset : float [-1.0..1.0 step 0.001]
}

struct Axle {
  index : integer [0..3]
}
```

```ridl
package veh.cluster

import veh.common.CalError
import veh.common.CalReport
import veh.common.Axle

interface Calibration {
  /// Fallible query — the inline `T | E` return is the canonical spelling.
  query calibrate(axle: Axle): CalReport | CalError
}
```

The left arm is the success type, the right arm exactly one `error` type.

One half of what follows from that you can see today: the Rust backend renders
the fallible return as a `Result<T, E>`, so exhaustive handling is enforced by
the compiler you already use. The other half is specified only — a binding is
required to split the union mechanically, sending the success arm as the reply
payload and mapping the error arm onto the transport's own error channel. No
binding exists to do it.

Two other failure kinds never appear in source at all, and both belong entirely
to that unbuilt layer. **Contract violations** — a payload outside its declared
range, or a failed `require` — are derived from the declarations you already
wrote, and the specification requires a runtime to report them uniformly rather
than making you declare them. **Transport failures** — timeouts, broker loss,
connection resets — are infrastructure failures the specification requires a
runtime to detect and leaves undeclared in the language, because a contract
author has no knowledge to express about them.

## Collections and streams

Collections are finite and always explicitly bounded:

```ridl
package veh.common

struct FaultCode {
  dtc : integer [0..65535]
}

struct DiagReport {
  faults   : [FaultCode; 0..32]       // bounded array — at most 32 faults
  sensors  : [Label : Speed; 1..8]    // bounded map — 1 to 8 entries
  readings : [Speed; 8]               // fixed array — exactly 8
  rawFrame : bytes [64]               // fixed 64-byte buffer
}
```

A **stream** `<T>` is the one unbounded container, and it is valid only in
interaction position — on a command or query parameter, or a query return. Its
element type is a named type describing one logical element; framing and
backpressure are transport concerns.

```ridl,allow=TYPL-115
package veh.common

type LogLine : string [1..1024] = "-"
type FwBlock : bytes  [1..65536]

struct SensorSample { value : Speed }
struct ProcessedSample { value : Speed }
struct DiagFilter { severity : Severity }
```

`FwBlock` draws a `TYPL-115` note: `bytes` with a minimum length above zero has
no derivable init value, exactly as a `string` with a minimum length above zero
or a `match` pattern does. The note is harmless here — a stream element is never
a signal payload, so nothing ever asks it for an init — and the block's fence
carries `allow=TYPL-115` to say so.

```ridl
package veh.cluster

import veh.common.LogLine
import veh.common.FwBlock
import veh.common.DiagFilter
import veh.common.FaultCode
import veh.common.SensorSample
import veh.common.ProcessedSample

interface Transfer {
  query   streamFaults(filter: DiagFilter): <FaultCode>       // provider produces
  query   streamLogs(): <LogLine>                             // text stream
  command uploadFirmware(data: <FwBlock>)                     // consumer produces
  query   pipe(samples: <SensorSample>): <ProcessedSample>    // bidirectional
}
```

Direction follows position. A stream in return position is declared to flow
from provider to consumer, one in parameter position the other way, and a query
with both declares full duplex. That is a statement about the contract; nothing
flows until a binding exists to carry it. A stream on a `signal` or an `event` is a `RIDL-201`
error — a signal is the better stream for state, and an unbounded push of
occurrences is an event.

## Contracts

`require` and `ensure` clauses go in an attribute block after a command or a
query. `require` is a precondition over the parameters and the interface's own
signals; `ensure` is a postcondition over `result`, and is valid on queries
only.

```ridl
package veh.cluster

import veh.common.Speed
import veh.common.SPEED_LIMIT_EU

interface Cruise {
  signal currentSpeed : Speed @10ms

  command setTargetSpeed(speed: Speed) [
    require speed > 0.0
    require speed <= SPEED_LIMIT_EU
  ]

  query getSpeedHistory(window: Duration): (min: Speed, max: Speed, avg: Speed) [
    require window > 0ms
    ensure  result.min <= result.avg
    ensure  result.avg <= result.max
  ]
}
```

Contract expressions are type-checked, unquoted, and side-effect-free. They are
restricted to a **guaranteed subset**: comparison, boolean connectives,
arithmetic, enum access, tuple-field access, and duration comparison, over
parameters, `result`, constants, enum values, and the interface's own signals.
Anything outside it is a `RIDL-306` error. Reading a field of a struct-typed
signal is outside the subset — publish the scalar you want to constrain as its
own signal instead.

The clauses are not documentation, and one use of them runs today: `ridl test`
draws sample parameter tuples and evaluates each `require` clause against them.
It does not evaluate `ensure` clauses — it lists them as observer stubs,
because checking a postcondition needs a result, and producing one needs the
runtime that does not exist yet. That same unbuilt layer is where the
specification puts the other use: a provider binding evaluating `require`
before your code runs, and `ensure` after it returns.

## Labels

`@labels` is a doc-comment tag carrying free-form classification identifiers —
safety, security, privacy, or any other domain-specific tag. The core language
defines no vocabulary:

```ridl
package veh.cluster

import veh.common.Speed

/**
 * Cruise control interface.
 * @labels SIL_2, SEC_2, PRIVATE
 */
interface CruiseControl {
  signal engagedSpeed : Speed @[50ms..500ms]
}

/**
 * Logging interface — no safety or security requirement.
 * @labels SIL_QM, SEC_NA, PUBLIC
 */
interface LogService {
  signal lastSpeed : Speed @[1s..10s]
}
```

Different domains use different vocabularies: an automotive profile would
define `ASIL_*`, a medical profile `IEC_62304_*`, an industrial profile
`IEC_61508_*`.

**As built, labels are carried through unchecked.** The compiler parses them
and passes them into the IR unchanged. The assurance profiles that would
validate a vocabulary and its combinations (diagnostics `TYPL-402` and
`TYPL-403`) are specified but not implemented.

## What the compiler produces

```sh
ridl build --emit rust --out-dir out
```

`ridl build` compiles a workspace and writes one artifact per package. Four
emit targets exist today:

| `--emit`     | Output              | Contents                                |
| ------------ | ------------------- | --------------------------------------- |
| `rust`       | `<package>.rs`      | idiomatic Rust source (the default)     |
| `c-header`   | `<package>.h`       | the extern-C header                     |
| `ir-json`    | `<package>.ir.json` | the lowered IR v2 as exact-decimal JSON |
| `typescript` | `<package>.ts`      | idiomatic TypeScript source             |

There is no transport binding and no code generator for SOME/IP, gRPC, DDS,
MQTT or AIDL yet. Those mappings are specified in the ridl language reference,
Appendix B, and are the work of later epics.

Classic CAN is not on that list, and never will be for a whole interface:
Appendix B records that DBC/CAN binds `signal` declarations only, and that an
`event`, `command` or `query` on classic CAN is a profile error. A bus carries
dataflow, not calls.

## The rest of the toolchain

| Command         | What it does                                                                             |
| --------------- | ---------------------------------------------------------------------------------------- |
| `ridl check`    | type-check a file, package, or workspace                                                 |
| `ridl build`    | compile to the artifacts above                                                           |
| `ridl fmt`      | rewrite `.typl` and `.ridl` files into one canonical form; `--check` reports without writing |
| `ridl baseline` | publish the current workspace as `.ridl/baseline/<package>.ir.json` snapshots            |
| `ridl diff`     | compare two IR snapshots or source trees and classify the change                         |
| `ridl test`     | run the property suite: range self-corpora, and sampling of `require` clauses. `ensure` clauses are listed as observer stubs, never evaluated |

`ridlc` is the plumbing underneath, with `check` and `build` only. Use `ridl`
unless you are scripting the compiler directly.

Two of those deserve a note.

**`ridl fmt` has its own canonical layout.** The listings in this chapter use
the aligned layout the language references use, which is not what the formatter
writes. Expect your files to change the first time you run it.

**`ridl diff` classifies, and its exit code carries the answer**: 0 when the
change is compatible or the snapshots are identical, 1 when it is breaking, 2
on an error. Interactions carry implicit ordinals by declaration order, so
appending is compatible and inserting or reordering is not. Retire an
interaction with a `reserved` tombstone rather than deleting it:

```ridl
package veh.cluster

import veh.common.Speed

// `DoorPayload` is declared earlier in this same package (`veh.cluster`), so
// it needs no import — everything in a package is visible to the rest of it.

interface VehicleStatus {
  signal currentSpeed : Speed @10ms
  reserved legacyTemp             // was ordinal 2 — retired
  event doorOpened : DoorPayload @[50ms..500ms]
}
```

`ridl check` also compares against a published baseline when
`.ridl/baseline/` exists, and reports a moved ordinal as a `RIDL-407` warning
at your desk, before CI sees it.

## Interfaces and services

An `interface` is an abstract shape: a reusable, identity-less group of
interactions. It is not addressable and not deployed.

A `service` is a global, named, published declaration of one — the catalog
entry that gives a contract a concrete identity in the system. The
specification defines its members as addressed `service.member`; no runtime
resolves such an address today, so read it as the naming scheme a deployment
will use. Service names are unique across the system and always public, and
that uniqueness the compiler does enforce (`RIDL-140`).

```ridl
package veh.cluster

import veh.common.Temperature

service veh.cluster.status : VehicleStatus

service veh.cluster.hvac {
  signal  temperature : Temperature @[1s..10s]
  command setTarget(temp: Temperature)
}
```

The first form names an existing interface; the second declares an inline shape
for a one-off contract not worth a reusable interface. A service declaration
says nothing about how the contract is realized on the wire — that is a
deployment question, and rsdl's, when rsdl exists.

Interfaces are flat: there is no interface inheritance. Sharing a _shape_ is
typl's job; a shared interaction set is not something the language supports, so
that a base change can never renumber a derived contract's wire identity.

## Naming conventions

| Construct                                     | Convention        | Example                        |
| --------------------------------------------- | ----------------- | ------------------------------ |
| `type`, `struct`, `enum`, `enumset`, `union`  | `CamelCase`       | `VehicleSpeed`, `WarningFlags` |
| Interfaces                                    | `CamelCase`       | `VehicleStatus`                |
| Interactions, fields, tuple fields, parameters | `camelCase`       | `currentSpeed`                 |
| Enum values, enumset bits, constants          | `SCREAMING_SNAKE` | `PARK`, `MAX_SPEED`            |
| Packages and service names                    | `lowercase.dot`   | `veh.common`                   |

Beyond spelling: name signals and finals as nouns (`currentSpeed`), events as
past-tense occurrences (`doorOpened`), commands as imperatives (`setGear`), and
queries as `get…` or `stream…`. A query named like a mutation draws a
`RIDL-404` warning, because it probably wanted to be a command.

Every family keyword is reserved in every profile, including keywords this
profile does not accept. `state`, `input`, `target`, `model`,
`component` and the rest of the registry are not available as identifiers even
in a `.ridl` file — the
typl language reference §1.4 lists the whole set.

## Next steps

- The [ridl language reference][ridl-ref] — the normative interaction layer.
- The [typl language reference][typl-ref] — the normative vocabulary layer.
- The [expr-core specification][expr-ref] — the grammar of `require` and
  `ensure`.
- The [family overview][overview] — the map, the shared doctrines, and the
  decision ledger.
- The four annexes below — complete, compiling interfaces from a vehicle
  domain.

[ridl-ref]: https://github.com/driftsys/ridl/blob/main/docs/specification/ridl-language-reference.md
[typl-ref]: https://github.com/driftsys/ridl/blob/main/docs/specification/typl-language-reference.md
[expr-ref]: https://github.com/driftsys/ridl/blob/main/docs/specification/expr-core-specification.md
[overview]: https://github.com/driftsys/ridl/blob/main/docs/specification/ridl-family-overview.md

---

## Annex 1 — Vehicle identity

A pure `final` interface: everything provisioned at the factory or over the
air. Note that every capability flag names a type rather than writing
`boolean`, and that `ModelYear` and `Speed` are declared types, not inline
ranges.

```ridl
package veh.identity

import veh.common.Speed

type ModelYear  : integer [2000..2100]
type Capability : boolean

enum FuelType    { PETROL = 0, DIESEL = 1, HYBRID = 2, ELECTRIC = 3, HYDROGEN = 4 }
enum DriveLayout { FWD = 0, RWD = 1, AWD = 2, FOUR_WD = 3 }

/**
 * Vehicle identity and hardware capability manifest.
 * Provisioned at the factory. Software fields updated over the air.
 *
 * @labels SIL_QM, SEC_2, CONFIDENTIAL
 */
interface VehicleIdentity {

  // vehicle identity — factory provisioned
  final vin          : Vin
  final modelYear    : ModelYear
  final marketRegion : Label
  final fuelType     : FuelType
  final driveLayout  : DriveLayout

  // software identity — updated over the air
  final softwareVersion   : Version
  final bootloaderVersion : Version
  final hardwareVersion   : Version

  // capability flags — build time
  final hasAdaptiveCruise   : Capability
  final hasEmergencyBraking : Capability
  final hasLaneKeepAssist   : Capability
  final hasParkingAssist    : Capability
  final hasDriverMonitoring : Capability
  final maxSupportedSpeed   : Speed
}
```

## Annex 2 — Powertrain

Two details worth reading closely. The engine's scalar speed is published as
its own `signal engineRpm`, because a `require` clause may read a signal but
not reach into a struct-typed one. And the engine state field is named
`engineState`: `state` is a reserved word family-wide, so it cannot be an
identifier.

```ridl,allow=RIDL-406
package veh.powertrain

type RPM         : /min  [0.0..8000.0 step 10.0]
type Torque      : N.m   [0.0..500.0 step 0.1]
type Temperature : Cel   [-40.0..150.0 step 0.1]
type FuelLevel   : %     [0.0..100.0 step 0.1]
type Voltage     : V     [0.0..48.0 step 0.1]
type Dtc         : integer [0..65535]
type FaultCount  : integer [0..65535]
type Severity    : integer [0..5]

enum EngineState {
  OFF      = 0
  CRANKING = 1
  RUNNING  = 2
  FAULT    = 3
}

enum GearPosition {
  PARK    = 0
  REVERSE = 1
  NEUTRAL = 2
  DRIVE   = 3
  LOW     = 4
}

struct EngineMetrics {
  rpm         : RPM
  torque      : Torque
  oilTemp     : Temperature
  coolantTemp : Temperature
  engineState : EngineState
}

struct TransmissionState {
  gear      : GearPosition
  inputRPM  : RPM
  outputRPM : RPM
  oilTemp   : Temperature
}

/// The `timestamp` field records when the fault occurred, which is domain
/// information distinct from the envelope's publication time. `ridl check`
/// notes the overlap (RIDL-406); the note is informational and the field is
/// the legitimate exception the rule names.
struct FaultPayload {
  dtc       : Dtc
  message   : Message
  timestamp : Timestamp
}

/**
 * Powertrain management interface.
 *
 * @labels SIL_3, SEC_2, PUBLIC
 */
interface PowertrainManager {

  signal engineMetrics     : EngineMetrics @20ms
  signal engineRpm         : RPM @20ms
  signal transmissionState : TransmissionState @[50ms..500ms]
  signal fuelLevel         : FuelLevel @[..5s]
  signal batteryVoltage    : Voltage @[100ms..1s]

  event engineFault       : FaultPayload @[100ms..30s]
  event transmissionFault : FaultPayload @[100ms..30s]

  command requestStart(mode: EngineState) [
    require mode == EngineState.CRANKING
  ]
  command requestStop()
  command setGear(gear: GearPosition) [
    require gear != GearPosition.PARK || engineRpm == 0.0
  ]

  query getDiagnostics(): (
    engineState   : EngineState
    activeGear    : GearPosition
    faultCount    : FaultCount
    lastFaultTime : Timestamp
  )

  query streamFaults(severity: Severity): <FaultPayload>

  final softwareVersion    : Version
  final calibrationVersion : Version
  final ecuSerial          : Uuid
  final supportedGears     : [GearPosition; 1..8]
}
```

## Annex 3 — Body control

A signal payload is a single named type, so a collection of door states is
published through a struct that wraps the array rather than as a bare array.
Command parameters name types too: `DoorIndex`, not `integer [0..7]`. And the
lock parameter is named `lock`, because `state` is reserved.

```ridl,allow=RIDL-406
package veh.body

import veh.common.Temperature

type Ratio      : %       [0.0..100.0 step 1.0]
type DoorIndex  : integer [0..7]
type DoorCount  : integer [1..8]
type WindowCount: integer [0..8]
type UnlockCount: integer [0..8]
type Zone       : integer [0..7]
type HasWindows : boolean
type AllLocked  : boolean
type Granted    : boolean

enum DoorPosition { CLOSED = 0, OPEN = 1, AJAR = 2 }
enum LockState    { LOCKED = 0, UNLOCKED = 1 }
enum LightState   { OFF = 0, ON = 1, FLASH = 2 }

struct DoorState {
  position : DoorPosition
  locked   : LockState
  sensorId : DoorIndex
}

struct WindowState {
  position : Ratio
  sensorId : DoorIndex
}

struct DoorSet {
  doors : [DoorState; 1..8]
}

struct WindowSet {
  windows : [WindowState; 1..8]
}

struct ClimateState {
  interiorTemp : Temperature
  targetTemp   : Temperature
  fanSpeed     : Ratio
  acActive     : boolean
}

/// `timestamp` records when access was attempted — domain time, not the
/// envelope's publication time (RIDL-406 note).
struct AccessEvent {
  zone      : Zone
  granted   : Granted
  method    : Label
  timestamp : Timestamp
}

/**
 * Body control module interface.
 * Covers doors, windows, lights and climate.
 *
 * @labels SIL_1, SEC_3, PRIVATE
 */
interface BodyControl {

  signal doorStates   : DoorSet @[50ms..2s]
  signal windowStates : WindowSet @[100ms..5s]
  signal climateState : ClimateState @[500ms..10s]

  event doorStateChanged : DoorState @[50ms..10s]

  /**
   * Vehicle access attempt — elevated privacy.
   * @labels SIL_1, SEC_4, CONFIDENTIAL
   */
  event accessAttempt : AccessEvent @[100ms..30s]

  command setAllLocks(lock: LockState)
  command setDoorLock(sensorId: DoorIndex, lock: LockState)
  command setWindowPosition(sensorId: DoorIndex, position: Ratio)
  command setClimateTarget(temp: Temperature) [
    require temp >= 16.0
    require temp <= 30.0
  ]

  query getLockStatus(): (allLocked: AllLocked, unlockedCount: UnlockCount)
  query getBodyStatus(): (doors: DoorSet, windows: WindowSet)

  final doorCount          : DoorCount
  final windowCount        : WindowCount
  final hasElectricWindows : HasWindows
}
```

## Annex 4 — Driver monitoring

```ridl,allow=RIDL-406
package veh.dms

type Ratio          : %       [0.0..100.0 step 0.1]
type SampleInterval : integer [50..1000]
type ZoneCount      : integer [1..16]
type Distracted     : boolean
type Fatigued       : boolean
type Monitoring     : boolean

enum AlertLevel {
  NONE     = 0
  LOW      = 1
  MEDIUM   = 2
  HIGH     = 3
  CRITICAL = 4
}

enum GazeZone {
  FORWARD      = 0
  MIRROR_LEFT  = 1
  MIRROR_RIGHT = 2
  MIRROR_REAR  = 3
  INSTRUMENT   = 4
  INFOTAINMENT = 5
  OFFROAD      = 6
  UNKNOWN      = 7
}

struct AttentionMetrics {
  gazeZone       : GazeZone
  eyeOpenness    : Ratio
  headPitch      : float [-30.0..30.0 step 0.1]
  headYaw        : float [-90.0..90.0 step 0.1]
  attentionScore : Ratio
}

struct DrowsinessState {
  level         : AlertLevel
  score         : Ratio
  eyeClosurePct : Ratio
}

/// `timestamp` records when the distraction occurred — domain time
/// (RIDL-406 note).
struct DistractionEvent {
  level     : AlertLevel
  gazeZone  : GazeZone
  duration  : Duration
  timestamp : Timestamp
}

/// `timestamp` records when the fatigue was detected — domain time
/// (RIDL-406 note).
struct FatigueEvent {
  level     : AlertLevel
  score     : Ratio
  timestamp : Timestamp
}

/**
 * Driver monitoring system interface.
 * Processes biometric and behavioural signals to assess driver state.
 *
 * @labels SIL_2, SEC_3, CONFIDENTIAL
 */
interface DriverMonitoring {

  signal attentionMetrics : AttentionMetrics @[100ms..500ms]
  signal drowsinessState  : DrowsinessState @[500ms..5s]

  event distractionDetected : DistractionEvent @[200ms..10s]
  event fatigueDetected     : FatigueEvent @[500ms..30s]

  command acknowledgeAlert(level: AlertLevel)
  command setMonitoringEnabled(enabled: Monitoring)

  query getDriverState(): (
    attentionScore  : Ratio
    drowsinessLevel : AlertLevel
    isDistracted    : Distracted
    isFatigued      : Fatigued
  ) [
    ensure result.attentionScore >= 0.0
    ensure result.attentionScore <= 100.0
  ]

  query streamAttention(interval: SampleInterval): <AttentionMetrics>

  final modelVersion  : Version
  final sensorType    : Label
  final gazeZoneCount : ZoneCount
}
```
