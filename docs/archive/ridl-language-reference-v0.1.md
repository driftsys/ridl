# RIDL Language Reference

**Reactive Interface Description Language** — transport-neutral reactive system
contract DSL for automotive software.

Version: 0.1.0 — Draft

---

## Table of Contents

1. [Lexical Conventions](#1-lexical-conventions)
2. [Package and Imports](#2-package-and-imports)
3. [Primitives](#3-primitives)
4. [Type Definitions](#4-type-definitions)
5. [Constants](#5-constants)
6. [Structs](#6-structs)
7. [Enums](#7-enums)
8. [Enum Sets](#8-enum-sets)
9. [Unions](#9-unions)
10. [Tuples](#10-tuples)
11. [Collections](#11-collections)
12. [Streams](#12-streams)
13. [Interfaces](#13-interfaces)
14. [Comments](#14-comments)
15. [Doc Comments](#15-doc-comments)
16. [Conventions](#16-conventions)
17. [Diagnostics](#17-diagnostics)

- [Appendix A — Standard Library](#appendix-a--standard-library)
- [Appendix B — Full Example](#appendix-b--full-example)
- [Appendix C — Standards References](#appendix-c--standards-references)
- [Appendix D — Codegen Targets](#appendix-d--codegen-targets)
- [Appendix E — Formal Grammar (EBNF)](#appendix-e--formal-grammar-ebnf)

---

## 1. Lexical Conventions

### 1.1 Encoding

RIDL source files are encoded in **UTF-8** as defined by
[Unicode Standard 15.0](https://www.unicode.org/versions/Unicode15.0.0/) and
[RFC 3629](https://www.rfc-editor.org/rfc/rfc3629). Non-ASCII characters are
only permitted inside string literals and doc comments.

### 1.2 Whitespace and Line Endings

Whitespace (space, tab, carriage return, newline) is insignificant except as a
token separator. Both Unix (`LF`) and Windows (`CRLF`) line endings are
accepted.

### 1.3 Identifiers

RIDL uses three identifier conventions, each restricted to ASCII:

| Form              | Pattern                              | Used for                                            |
| ----------------- | ------------------------------------ | --------------------------------------------------- |
| `CamelCase`       | Starts with uppercase, no separators | Types, Structs, Enums, EnumSets, Unions, Interfaces |
| `camelCase`       | Starts with lowercase, no separators | Fields, interaction names, parameters, tuple fields |
| `SCREAMING_SNAKE` | Uppercase, underscore separators     | Enum values, EnumSet bits, constants                |

Identifiers must start with a letter. Digits are permitted after the first
character. Underscore is permitted only in `SCREAMING_SNAKE` form.

Reserved keywords: `package`, `import`, `as`, `type`, `const`, `struct`, `enum`,
`enumset`, `union`, `interface`, `signal`, `event`, `command`, `query`, `final`,
`boolean`, `integer`, `float`, `string`, `bytes`, `true`, `false`, `step`,
`match`.

### 1.4 Integer Literals

Follow [ISO/IEC 9899:2018 (C17)](https://www.iso.org/standard/74528.html)
§6.4.4.1:

- Decimal: `0`, `42`, `65535`
- No leading zeros
- Negative values with unary minus: `-40`

### 1.5 Floating-Point Literals

Follow [IEEE 754-2019](https://ieeexplore.ieee.org/document/8766229) double
precision:

- Must include a decimal point: `0.0`, `3.14`, `-40.0`
- Scientific notation not supported in v0.1
- `NaN` and `Inf` not permitted in range constraints

### 1.6 String Literals

UTF-8 sequences enclosed in double quotes. Escape sequences follow
[JSON RFC 8259 §7](https://www.rfc-editor.org/rfc/rfc8259): `\"`, `\\`, `\n`,
`\r`, `\t`, `\uXXXX`.

### 1.7 Regex Literals

Enclosed in forward slashes. Syntax follows
[ECMA-262](https://tc39.es/ecma262/#sec-regexp-regular-expression-objects).
Forward slashes inside the pattern must be escaped: `\/`.

```ridl
/^[A-HJ-NPR-Z0-9]{17}$/
/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
```

### 1.8 Duration Literals

A positive integer followed by a time unit suffix. Used exclusively in timing
annotations.

| Suffix | Unit         | Example |
| ------ | ------------ | ------- |
| `us`   | Microseconds | `500us` |
| `ms`   | Milliseconds | `10ms`  |
| `s`    | Seconds      | `1s`    |

Zero duration is not permitted. Fractions are not supported — use a smaller unit
(`500us` not `0.5ms`).

---

## 2. Package and Imports

### 2.1 Package Declaration

Every RIDL file begins with exactly one `package` declaration. The package name
is a dot-separated sequence of lowercase identifiers following reverse-domain
convention:

```ridl
package veh.cluster
package veh.powertrain
package veh.adas
```

### 2.2 Import Declaration

Types and constants defined in other packages must be imported before use:

```ridl
import veh.common.Speed                  // single type
import veh.common.Speed as CommonSpeed   // alias — resolves name conflicts
import veh.common.*                      // wildcard — linter warning
```

**Implicit imports** — always available without explicit import:

- All definitions in the same package
- All definitions in `ridl.std`

### 2.3 Qualified References

Any definition may be referenced by its fully qualified name without an import:

```ridl
signal engineTemp: veh.thermal.Temperature @[20ms..100ms]
```

---

## 3. Primitives

RIDL defines five primitive types. Primitives are lowercase keywords — visually
distinct from `CamelCase` named types. They are used as backing types in `type`
definitions. Direct use as field types is restricted — see
[§16.3](#163-type-usage).

| Primitive | Meaning                                                  | Constraint                        |
| --------- | -------------------------------------------------------- | --------------------------------- |
| `boolean` | logical true/false                                       | none                              |
| `integer` | whole number — width inferred from range                 | recommended — profile may require |
| `float`   | real number — width inferred from range and step         | recommended — profile may require |
| `string`  | character sequence — default `[0..256]` if unspecified   | recommended — profile may require |
| `bytes`   | opaque binary buffer — default `[0..256]` if unspecified | recommended — profile may require |

### 3.1 Boolean

Single logical value. No constraint syntax.

```ridl
isOpen   : boolean
isActive : boolean
```

Serialized as `uint8` on transport — `0` = false, `1` = true.

### 3.2 Integer

Whole number. Range constraint `[min..max]` strongly recommended — active
profiles may require it.

```ridl
type Counter : integer [0..255]
type Index   : integer [0..65535]
type Signed  : integer [-128..127]
```

Width inferred by the RIDL compiler from the declared range — applied
consistently across all targets:

| Range                       | Wire type |
| --------------------------- | --------- |
| `[0..255]`                  | `uint8`   |
| `[-128..127]`               | `int8`    |
| `[0..65535]`                | `uint16`  |
| `[-32768..32767]`           | `int16`   |
| `[0..4294967295]`           | `uint32`  |
| `[-2147483648..2147483647]` | `int32`   |
| larger unsigned             | `uint64`  |
| larger signed or no range   | `int64`   |

**Language layer** — always `int64`. **Transport layer** — narrowest wire type
that safely contains the range.

### 3.3 Float

Real number. Range and `step` strongly recommended — active profiles may require
them.

```ridl
type Gain      : float [0.0..1.0 step 0.01]
type Precision : float [0.0..1.0 step 0.000001]
```

| Step                      | Wire type |
| ------------------------- | --------- |
| `step >= 0.001`           | `float32` |
| `step < 0.001` or no step | `float64` |

**Language layer** — always `float64`. **Transport layer** — narrowest that
fits.

### 3.4 String

Character sequence. Bound is in characters. Encoding (ASCII, UTF-8, UTF-16) is a
codegen concern per target. Default bounds `[0..256]` apply when unspecified —
active profiles may require explicit bounds.

```ridl
type Vin     : string [17 match VIN_PATTERN]      // fixed 17 characters
type Label   : string [1..64 match ASCII_PATTERN] // 1 to 64 characters
type Name    : string [1..128]                    // 1 to 128 characters
type Comment : string                             // default [0..256]
```

`string` must not be used directly as a field type — always define a named
`type`. Exception: `string` is permitted as a stream element type — see
[§12](#12-streams).

### 3.5 Bytes

Opaque binary buffer. Constraint syntax mirrors `string` — bound is in bytes.
Default bounds `[0..256]` apply when unspecified — active profiles may require
explicit bounds. No `match` pattern.

```ridl
type CanFrame    : bytes [8]         // fixed 8 bytes
type Certificate : bytes [1..4096]   // variable bounded
type Sha256Hash  : bytes [32]        // fixed 32 bytes
type Payload     : bytes             // default [0..256]
```

`bytes` must not be used directly as a field type — always define a named
`type`. Exception: `bytes` is permitted as a stream element type — see
[§12](#12-streams).

---

## 4. Type Definitions

The `type` keyword defines a named scalar — a primitive or physical unit type
with optional constraints. Named types carry domain meaning and are the primary
building blocks for all fields and interactions.

### 4.1 Physical Unit Types

Physical unit types associate a scalar value with a unit from the **Unified Code
for Units of Measure (UCUM)**:

> UCUM is defined by the [Regenstrief Institute](https://ucum.org/ucum) and is
> the unit standard used by
> [OpenTelemetry](https://opentelemetry.io/docs/specs/semconv/general/metrics/),
> [HL7 FHIR](https://hl7.org/fhir/valueset-ucum-units.html), and
> [ISO 11240](https://www.iso.org/standard/55032.html).

```ridl
type Speed       : km/h  [0.0..250.0 step 0.5]
type Temperature : Cel   [-40.0..125.0 step 0.1]
type Torque      : N.m   [0.0..500.0 step 0.1]
type RPM         : /min  [0.0..8000.0 step 10.0]
type Pressure    : bar   [0.0..10.0 step 0.01]
type Voltage     : V     [0.0..48.0 step 0.1]
type Ratio       : %     [0.0..100.0 step 0.1]
```

The underlying primitive is `float`. UCUM unit expressions are
**case-sensitive**.

Common automotive UCUM units:

| UCUM   | Meaning                   |
| ------ | ------------------------- |
| `km/h` | Kilometres per hour       |
| `Cel`  | Degrees Celsius           |
| `N.m`  | Newton metres             |
| `/min` | Revolutions per minute    |
| `m/s2` | Metres per second squared |
| `bar`  | Bar (pressure)            |
| `V`    | Volt                      |
| `A`    | Ampere                    |
| `W`    | Watt                      |
| `%`    | Ratio / percentage        |

### 4.2 Constrained Primitive Types

```ridl
type Counter : integer [0..65535]
type Gain    : float   [0.0..1.0 step 0.01]
type Vin     : string  [17 match VIN_PATTERN]
type Frame   : bytes   [8]
```

### 4.3 String Constraint Syntax

| Syntax                            | Meaning                 |
| --------------------------------- | ----------------------- |
| `string [N]`                      | fixed N characters      |
| `string [min..max]`               | min to max characters   |
| `string [N match PATTERN]`        | fixed N with validation |
| `string [min..max match PATTERN]` | range with validation   |

The `match` keyword references a named regex constant or an inline regex
literal.

### 4.4 Bytes Constraint Syntax

| Syntax             | Meaning          |
| ------------------ | ---------------- |
| `bytes [N]`        | fixed N bytes    |
| `bytes [min..max]` | min to max bytes |

No `match` — bytes are opaque binary.

---

## 5. Constants

The `const` keyword defines a named compile-time constant. Package-scoped and
importable like types. Reusable in range constraints, `match` patterns,
`require`/`ensure` expressions, and `default` values.

### 5.1 Value Constants

```ridl
package veh.common

const MAX_SPEED      : Speed   = 250.0
const SPEED_LIMIT_EU : Speed   = 130.0
const SPEED_LIMIT_US : Speed   = 121.0
const MAX_GEAR       : integer = 6
const IDLE_RPM       : RPM     = 800.0
```

`SCREAMING_SNAKE` naming — visually distinct from types (`CamelCase`) and fields
(`camelCase`).

Reused in type ranges, struct constraints, and contracts:

```ridl
type Speed : km/h [0.0..MAX_SPEED step 0.5]

struct GearState {
  gear : integer [0..MAX_GEAR]
}

command setTargetSpeed(speed: Speed) [
  require speed <= SPEED_LIMIT_EU
]
```

### 5.2 Regex Constants

A `const` may hold a regex literal — a compiled pattern reusable in `match`
constraints:

```ridl
const VIN_PATTERN   = /^[A-HJ-NPR-Z0-9]{17}$/
const UUID_PATTERN  = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const ASCII_PATTERN = /^[\x20-\x7E]+$/
```

Referenced by name or used inline:

```ridl
type Vin   : string [17 match VIN_PATTERN]
type Email : string [1..254 match /^[^@]+@[^@]+\.[^@]+$/]
```

---

## 6. Structs

A `struct` defines a named composite type with a fixed set of named fields.

```ridl
struct SpeedLimitPayload {
  limit  : Speed
  actual : Speed
}

struct DriverProfile {
  name     : Name
  speed    : Speed
  override : Speed?     // optional — may be absent
}
```

The `?` suffix marks a field as optional. Maps to `optional` in proto3,
`Option<T>` in Rust, nullable `T?` in Kotlin.

Newline or comma are interchangeable as field separators:

```ridl
// newline style — preferred
struct Point {
  x : float
  y : float
  z : float
}

// comma style — compact inline
struct Point { x: float, y: float, z: float }
```

---

## 7. Enums

An `enum` defines a named set of discrete integer-backed values. Used for
single-value selection.

```ridl
enum GearPosition {
  PARK    = 0
  DRIVE   = 1
  REVERSE = 2
  NEUTRAL = 3
}
```

- Backing type is `integer`, implicit
- All values must be explicitly assigned
- Values must be unique within the enum
- First value conventionally `= 0` for proto3 compatibility

---

## 8. Enum Sets

An `enumset` defines a named bitfield — a set of named flags where multiple
values may be active simultaneously. Backing width inferred by the compiler from
the highest bit position.

### 8.1 Standalone Form

Bits defined directly — preferred when flags are only ever used as a set:

```ridl
enumset WarningFlags {
  LOW_FUEL     = 0    // bit 0
  CHECK_ENGINE = 1    // bit 1
  DOOR_OPEN    = 2    // bit 2
  SEATBELT     = 3    // bit 3
}
```

### 8.2 Derived Form

Derives bit positions from an existing `enum` — preferred when both single-value
and multi-value use are needed:

```ridl
enum Warning {
  LOW_FUEL     = 0
  CHECK_ENGINE = 1
  DOOR_OPEN    = 2
  SEATBELT     = 3
}

enumset WarningFlags : Warning
```

Derived form allows both single and set usage:

```ridl
command clearWarning(flag: Warning)        // single flag
signal activeWarnings: WarningFlags @50ms  // multiple flags simultaneously
```

### 8.3 Width Inference

| Highest bit | Wire type |
| ----------- | --------- |
| 0..7        | `uint8`   |
| 8..15       | `uint16`  |
| 16..31      | `uint32`  |
| 32..63      | `uint64`  |

Language layer — always `int64`. Transport layer — narrowest inferred wire type.

---

## 9. Unions

A `union` defines a discriminated (tagged) union — exactly one arm is active at
any time.

```ridl
union SensorResult {
  ok  : SensorReading
  err : SensorFault
}
```

- Arms must reference **named types** only — primitives not permitted directly
- Maps to `oneof` in proto3, `union` in FlatBuffers, `sealed class` in Kotlin

---

## 10. Tuples

A tuple is an anonymous set of named fields used inline — as interaction
parameters, query return types, and struct fields.

```ridl
// query return
query getMinMax(window: Duration): (min: Speed, max: Speed)

// command parameters
command setRange(min: Speed, max: Speed)

// struct field
struct SensorBounds {
  range : (min: Speed, max: Speed)
}
```

- Fields are always **named** — positional access not permitted
- The empty tuple `()` is the unit/void type — implicit return of every
  `command`
- `query` must not return `()` — compiler error, use `command`
- `require`/`ensure` access tuple fields by name: `result.min <= result.max`
- Same tuple shape used in multiple places → linter warning: extract as named
  `struct`

---

## 11. Collections

Collections are finite, bounded aggregates. All variable-size collections
require explicit bounds — compiler error if missing.

### 11.1 Array

An ordered sequence of elements of the same type.

```ridl
// fixed array — exactly N elements
readings : [Speed; 8]

// bounded array — min to max elements
faults   : [FaultCode; 0..32]
labels   : [Label; 1..16]
```

### 11.2 Map

A key-value collection. Keys must be a named string type or primitive.

```ridl
// bounded map
metadata : [Label : Name; 0..32]
sensors  : [Label : Speed; 1..8]
config   : [Label : float; 1..16]
```

### 11.3 Bound Rules

Collection bounds are always required and must be explicit — there are no
defaults. Streams are the only construct with no declared bound; transport
decides framing and packetization.

| Construct     | Syntax            | Bound                           |
| ------------- | ----------------- | ------------------------------- |
| Fixed array   | `[T; N]`          | exactly N elements — mandatory  |
| Bounded array | `[T; min..max]`   | min to max elements — mandatory |
| Bounded map   | `[K:V; min..max]` | min to max entries — mandatory  |
| Stream        | `<T>`             | none — transport decides        |

---

## 12. Streams

A stream is an unbounded sequence of elements — size unknown until the stream is
closed. Streams are transport-driven: framing, buffering, MTU, and backpressure
are transport concerns. Streams are only valid on `command` and `query`
interactions.

```ridl
query streamFaults(filter: DiagFilter): <FaultEvent>      // server produces
command uploadFirmware(data: <FwBlock>)                    // client produces
query pipe(input: <SensorSample>): <ProcessedSample>      // bidirectional
```

### 12.1 Direction

Direction is determined by position — no keyword needed:

| Position    | Direction                        |
| ----------- | -------------------------------- |
| Return type | server produces, client consumes |
| Parameter   | client produces, server consumes |
| Both        | bidirectional, full-duplex       |

### 12.2 Element Type

The element type is always a named type, `string`, or `bytes`. `string` and
`bytes` are permitted as stream element types as an exception to the field type
rule — streams are unbounded by nature, so the normal bound requirement does not
apply to the stream container itself.

```ridl
type LogLine : string [1..1024]      // one logical log entry — named type
type FwBlock : bytes [1..65536]      // one logical firmware block — named type

query streamLogs(): <LogLine>             // named type stream
query streamLogs(): <string>             // raw string stream — permitted
command uploadFirmware(data: <FwBlock>)  // named type stream
command uploadFirmware(data: <bytes>)    // raw bytes stream — permitted
query streamFaults(): <FaultEvent>       // structured stream
```

Named types are preferred — they carry domain meaning and bounds. Raw `string`
and `bytes` are permitted when the element is genuinely unstructured.

### 12.3 No Chunk Bound

Streams carry no chunk size or total bound — transport decides packetization and
framing. The element type bound describes one logical element, not a transport
frame.

Maps to: gRPC streaming, Kotlin `Flow`, Rust `AsyncRead`/`AsyncWrite`, vsock,
SOME/IP event group.

---

## 13. Interfaces

An `interface` groups related interactions under a named contract. Pure contract
definition — does not imply a deployment or runtime unit. Inspired by
[CORBA IDL](https://www.omg.org/spec/I2C/),
[Franca IDL](https://franca.sourceforge.net/), and
[Android AIDL](https://developer.android.com/guide/components/aidl).

RIDL defines five interaction kinds:

| Keyword   | Kind        | Semantic                       |
| --------- | ----------- | ------------------------------ |
| `signal`  | pub/sub     | continuous volatile value      |
| `event`   | pub/sub     | discrete occurrence            |
| `command` | RPC         | fire-and-forget                |
| `query`   | RPC         | request/response               |
| `final`   | provisioned | immutable for runtime lifetime |

```ridl
/**
 * Main vehicle status interface.
 *
 * @see veh.powertrain.PowertrainInterface
 * @labels SIL_B, CAL_2, PRIVATE
 */
interface VehicleStatus {

  signal currentSpeed: Speed @10ms
  signal engineTemp: Temperature @[20ms..100ms]
  signal warnings: WarningFlags @[50ms..1s]

  event doorOpened: DoorPayload @[50ms..500ms]
  event speedLimitExceeded: SpeedLimitPayload @[100ms..2000ms]

  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]
  command resetFaults()

  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ]
  query getMinMax(window: Duration): (min: Speed, max: Speed) [
    require window > 0ms
    ensure  result.min <= result.max
  ]
  query streamFaults(filter: DiagFilter): <FaultEvent>

  final softwareVersion: Version
  final ecuSerial: Uuid
  final capabilities: [Label; 0..32]
}
```

### 13.1 Signal

A **continuous volatile value**. The latest sample is what matters — missing an
intermediate sample is acceptable.

```ridl
signal currentSpeed: Speed @10ms
signal engineTemp: Temperature @[20ms..100ms]
```

- Single named type payload
- Timing annotation mandatory
- Valid timing: `@Xms` (strict periodic) or `@[min..max]` (debounce/refresh)
- Stream `<T>` not valid on signals
- Maps to AUTOSAR `SenderReceiverInterface` with `isQueued = false`

### 13.2 Event

A **discrete occurrence**. Every occurrence matters.

```ridl
event doorOpened: DoorPayload @[50ms..500ms]
event speedLimitExceeded: SpeedLimitPayload @[100ms..2000ms]
```

- Single named type payload
- Timing annotation mandatory
- Valid timing: `@[min..max]` only — strict periodic `@Xms` not valid on events
- Stream `<T>` not valid on events
- Maps to AUTOSAR `SenderReceiverInterface` with `isQueued = true`

### 13.3 Timing Annotations

`@` prefix — used **exclusively** for timing throughout RIDL.

**Strict periodic — signal only:**

```ridl
signal currentSpeed: Speed @10ms
```

**Range — signal and event:**

```ridl
@[20ms..100ms]    // both bounds
@[20ms..]         // lower bound only
@[..100ms]        // upper bound only
```

Semantics by construct:

| Bound       | Signal                                             | Event                                                       |
| ----------- | -------------------------------------------------- | ----------------------------------------------------------- |
| lower `min` | **debounce** — suppress updates faster than `min`  | **throttle** — min interval between occurrences             |
| upper `max` | **refresh ceiling** — re-publish even if unchanged | **TTL** — discard if processed after `max` from when raised |

### 13.4 Command

**Fire-and-forget** RPC. Always returns `()` — no return type written.

```ridl
command setGear(position: GearPosition)
command setRange(min: Speed, max: Speed)
command uploadFirmware(data: <FwBlock>)
command resetFaults()
```

- Stream `<T>` permitted on parameters
- Maps to proto unary RPC with `google.protobuf.Empty` response

### 13.5 Query

**Request/response** RPC. Always returns a non-void result.

```ridl
query getAverageSpeed(window: Duration): Speed
query getMinMax(window: Duration): (min: Speed, max: Speed)
query streamFaults(filter: DiagFilter): <FaultEvent>
query pipe(input: <SensorSample>): <ProcessedSample>
```

- Must return non-void — `()` return is a compiler error, use `command`
- Stream `<T>` permitted on parameters and return type
- Maps to Kotlin `suspend fun`, proto unary or streaming RPC, SOME/IP method

### 13.6 Final

A value provisioned externally — at build time, factory, or FOTA — and
**immutable for the lifetime of the running software instance**.

```ridl
interface VehicleIdentity {
  final vin: Vin
  final softwareVersion: Version
  final marketRegion: Label
  final ecuSerial: Uuid
  final capabilities: [Label; 0..32]
}
```

- Single named type payload
- No timing annotation
- No attribute block
- Read-only — no command can mutate it
- Safe to cache — stable for the software instance lifetime
- Updated between instances via FOTA or factory provisioning
- Maps to `ro.*` Android system properties, AUTOSAR `CalibrationParameter`,
  SOME/IP field with getter only

### 13.7 Attribute Blocks

`[ ]` block after the interaction declaration. Newline or comma interchangeable
as separators.

```ridl
command setGear(position: GearPosition) [
  require position != GearPosition.PARK || currentSpeed == 0.0
]

query getAverageSpeed(window: Duration): Speed [
  require window > 0ms
  ensure  result >= 0.0
]
```

| Attribute           | Meaning                                              | Valid on                  |
| ------------------- | ---------------------------------------------------- | ------------------------- |
| `default = <value>` | Initial or reset value                               | struct fields, parameters |
| `require <expr>`    | Precondition — type-checked, unquoted                | `command`, `query`        |
| `ensure <expr>`     | Postcondition over `result` — type-checked, unquoted | `query` only              |

Inspired by **Design by Contract** —
[Eiffel](https://www.eiffel.com/values/design-by-contract/introduction/) and
[SPARK Ada](https://docs.adacore.com/spark2014-docs/html/ug/en/source/contract_based_programming.html).

---

## 14. Comments

All comments are discarded by the compiler.

**Line comment:**

```ridl
// this is a line comment
signal currentSpeed: Speed @10ms  // inline
```

**Block comment — nesting not supported:**

```ridl
/* this is a
   block comment */
```

---

## 15. Doc Comments

Doc comments are attached to the immediately following definition. Processed by
documentation generators and IDEs — no semantic effect on compiler or codegen.
Inspired by [KDoc/Dokka](https://kotlinlang.org/docs/kotlin-doc.html) and
[Rustdoc](https://doc.rust-lang.org/rustdoc/). No blank lines between a doc
comment and its definition.

### 15.1 Syntax

**Single line** — `///` prefix:

```ridl
/// Current vehicle speed published at strict 10ms rate
signal currentSpeed: Speed @10ms
```

**Multi-line** — `/** */` delimiters with full
[CommonMark](https://commonmark.org) markdown:

````ridl
/**
 * Multi-line doc comment — full CommonMark markdown supported.
 *
 * Supports **bold**, _italic_, `inline code`.
 *
 * Reference links resolve to generated documentation pages:
 * - [Speed] — links to the Speed type in the same package
 * - [VehicleStatus] — links to the interface
 * - [veh.common.Temperature] — fully qualified cross-package reference
 *
 * External links — [AUTOSAR SOME/IP](https://www.autosar.org)
 *
 * Code block:
 * ```ridl
 * signal currentSpeed: Speed @10ms
 * signal engineTemp: Temperature @[20ms..100ms]
 * ```
 *
 * @see veh.powertrain.PowertrainInterface
 * @labels SIL_B, CAL_2, PRIVATE
 * @deprecated "use VehicleStatusV2 instead"
 */
interface VehicleStatus { ... }
````

### 15.2 Tags

Three tags only — always placed after the markdown body:

| Tag           | Value                                 | Valid on |
| ------------- | ------------------------------------- | -------- |
| `@see`        | qualified type or interface name      | all      |
| `@labels`     | comma-separated classification labels | all      |
| `@deprecated` | `"reason string"`                     | all      |

### 15.3 Labels

`@labels` carries free-form classification identifiers. The core language
defines no vocabulary — labels are opaque strings validated by an external
**profile**. A profile declares which labels are valid, which combinations are
required, and what constraints they impose on the interface.

```ridl
// automotive profile vocabulary
/** @labels SIL_B, CAL_2, PRIVATE */
interface VehicleStatus { ... }

// medical profile vocabulary
/** @labels SIL_2, IEC_62443, PHI */
interface PatientMonitor { ... }

// industrial profile vocabulary
/** @labels PLd, IEC_61508, RESTRICTED */
interface SafetyRelay { ... }

// no labels — profile may warn or error
interface LogService { ... }
```

The compiler passes labels through to generated metadata unchanged. Profile
plug-ins attached to `ridlc` perform vocabulary validation and enforce
constraints such as required label combinations, incompatible label pairs, or
mandatory labels for specific interaction kinds.

**Label identifiers** follow `SCREAMING_SNAKE` convention. Parenthesised
suffixes are permitted — e.g. `MY_LABEL(D)`.

### 15.4 Reference Links

`[TypeName]` resolves within the same package. `[pkg.TypeName]` resolves across
packages. Unresolved references produce a linter warning.

---

## 16. Conventions

### 16.1 Naming

| Construct                                                 | Convention        | Example                         |
| --------------------------------------------------------- | ----------------- | ------------------------------- |
| `type`, `struct`, `enum`, `enumset`, `union`, `interface` | `CamelCase`       | `VehicleSpeed`, `WarningFlags`  |
| Fields, interactions, parameters, tuple fields            | `camelCase`       | `currentSpeed`, `setGear`       |
| Enum values, enumset bits, constants                      | `SCREAMING_SNAKE` | `PARK`, `LOW_FUEL`, `MAX_SPEED` |
| Packages                                                  | `lowercase.dot`   | `veh.cluster`                   |

### 16.2 Separators

Newline and comma are interchangeable as separators in `struct`, `enum`,
`enumset`, `union`, tuple, and `[]` attribute blocks. Trailing comma permitted.

### 16.3 Type Usage

- `string` and `bytes` must not be used directly as field types — always define
  a named `type`
- All variable-size types must declare explicit bounds — no defaults, no
  exceptions
- `union` arms must reference named types only
- `query` must not return `()` — use `command`
- Prefer named types over inline primitives for all domain concepts

### 16.4 Imports

Prefer explicit single-type imports over wildcards. Wildcard imports obscure
type dependencies, making safety analysis and impact assessment harder.

---

## 17. Diagnostics

Compiler errors prevent compilation. Linter warnings are informational.

### 17.1 Package and Import

| Rule                                             | Severity |
| ------------------------------------------------ | -------- |
| More than one `package` declaration per file     | error    |
| Wildcard import                                  | warning  |
| Unused import                                    | warning  |
| Conflicting imports without alias                | error    |
| Circular package imports                         | error    |
| Unresolved `[TypeName]` reference in doc comment | warning  |

### 17.2 Types and Structures

| Rule                                                          | Severity |
| ------------------------------------------------------------- | -------- |
| `integer` without range                                       | warning  |
| `integer` without range — active profile requires it          | error    |
| `float` without range + `step`                                | warning  |
| `float` without range + `step` — active profile requires it   | error    |
| `string` without explicit bounds — default `[0..256]` applied | warning  |
| `string` without explicit bounds — active profile requires it | error    |
| `bytes` without explicit bounds — default `[0..256]` applied  | warning  |
| `bytes` without explicit bounds — active profile requires it  | error    |
| Array `[T; N]` or `[T; min..max]` without explicit bounds     | error    |
| Map `[K:V; min..max]` without explicit bounds                 | error    |
| `string` used directly as field type                          | error    |
| `bytes` used directly as field type                           | error    |
| Range `min > max`                                             | error    |
| `step` type mismatch                                          | error    |
| `union` arm with primitive type                               | error    |
| Enum values not unique                                        | error    |
| EnumSet bit positions not unique                              | error    |
| `default` value incompatible with field type                  | error    |
| Same tuple shape used in multiple places                      | warning  |
| Invalid regex syntax in `match` or `const`                    | error    |
| Regex contradicts declared character bound                    | warning  |

### 17.3 Interactions

| Rule                                                   | Severity |
| ------------------------------------------------------ | -------- |
| `signal` or `event` without timing annotation          | warning  |
| `@Xms` strict periodic on `event`                      | error    |
| `@[X..Y]` where `X > Y`                                | error    |
| `@[X..X]` — equivalent to `@Xms`                       | warning  |
| `@0ms` — duration must be positive                     | error    |
| `<T>` stream on `signal` or `event`                    | error    |
| `<T>` stream on struct field or collection             | error    |
| `query` returning `()`                                 | error    |
| `command` with explicit return type                    | error    |
| `ensure` on `command`                                  | error    |
| `require` or `ensure` on `signal`, `event`, or `final` | error    |
| Timing annotation on `final`                           | error    |
| Attribute block on `final`                             | error    |

### 17.4 Doc Comments

| Rule                                                  | Severity |
| ----------------------------------------------------- | -------- |
| `@labels` identifier not recognised by active profile | info     |
| `@labels` combination invalid per active profile      | error    |
| Blank line between doc comment and definition         | warning  |
| `@deprecated` without reason string                   | warning  |

---

## Appendix A — Standard Library

The `ridl.std` package is implicitly imported in every RIDL file. All
definitions are available without explicit import.

```ridl
package ridl.std

// ---------- Regex Constants ----------

const UUID_PATTERN  = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const ULID_PATTERN  = /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/
const VIN_PATTERN   = /^[A-HJ-NPR-Z0-9]{17}$/
const URI_PATTERN   = /^[a-zA-Z][a-zA-Z0-9+\-.]*:\/\/.+$/
const URL_PATTERN   = /^https?:\/\/.+$/
const EMAIL_PATTERN = /^[^@]+@[^@]+\.[^@]+$/
const IPV4_PATTERN  = /^(\d{1,3}\.){3}\d{1,3}$/
const IPV6_PATTERN  = /^[0-9a-f:]+$/
const ASCII_PATTERN = /^[\x20-\x7E]+$/

// ---------- Identity Types ----------

/// RFC 4122 UUID — 36 characters including hyphens
type Uuid : string [36 match UUID_PATTERN]

/// ULID — 26 characters, lexicographically sortable
type Ulid : string [26 match ULID_PATTERN]

/// ISO 3779 Vehicle Identification Number — exactly 17 characters
type Vin  : string [17 match VIN_PATTERN]

// ---------- Network Types ----------

/// RFC 3986 URI
type Uri   : string [1..2048 match URI_PATTERN]

/// HTTP/HTTPS URL
type Url   : string [1..2048 match URL_PATTERN]

/// Email address — RFC 5321
type Email : string [1..254 match EMAIL_PATTERN]

/// IPv4 address — dotted decimal notation
type IpV4  : string [7..15 match IPV4_PATTERN]

/// IPv6 address
type IpV6  : string [2..39 match IPV6_PATTERN]

// ---------- General String Types ----------

/// Short human-readable label — ASCII printable, max 64 characters
type Label   : string [1..64 match ASCII_PATTERN]

/// Human-readable name — max 128 characters
type Name    : string [1..128 match ASCII_PATTERN]

/// Short message or description — max 256 characters
type Message : string [1..256 match ASCII_PATTERN]

// ---------- Time Types ----------

/// Absolute point in time — RFC 3339 / ISO 8601
/// Backing: int64 Unix timestamp in microseconds since epoch
type Timestamp : integer [0..9223372036854775807]

/// Elapsed time interval in milliseconds
type Duration  : ms [0.0..9223372036854775807]

/// Calendar date — ISO 8601 YYYY-MM-DD
type Date : string [10 match /^\d{4}-\d{2}-\d{2}$/]

/// Time of day — ISO 8601 HH:MM:SS
type TimeOfDay : string [8..15 match /^\d{2}:\d{2}:\d{2}/]

// ---------- Version ----------

/// Semantic version — SemVer 2.0.0
type Version : string [5..32 match /^\d+\.\d+\.\d+/]

// ---------- Locale ----------

/// ISO 3166-1 alpha-2 country code
type CountryCode : string [2 match /^[A-Z]{2}$/]

/// ISO 639-1 language code with optional region — e.g. "en", "en-US"
type LanguageCode : string [2..5 match /^[a-z]{2}(-[A-Z]{2})?$/]

// ---------- Crypto / Security ----------

/// SHA-256 hash — 32 bytes
type Sha256Hash  : bytes [32]

/// SHA-512 hash — 64 bytes
type Sha512Hash  : bytes [64]

/// Generic cryptographic signature — 64 bytes
type Signature   : bytes [64]

/// X.509 DER certificate
type Certificate : bytes [1..4096]
```

---

## Appendix B — Full Example

````ridl
package veh.cluster

import veh.common.Speed
import veh.common.Temperature
import veh.common.MAX_SPEED
import veh.common.SPEED_LIMIT_EU

type Counter : integer [0..65535]

struct SpeedLimitPayload {
  limit  : Speed
  actual : Speed
}

struct DoorPayload {
  sensorId : integer [0..15]
  isOpen   : boolean
}

struct DiagFilter {
  severity : integer [0..5]
  category : Label?
}

struct FaultEvent {
  code      : integer [0..65535]
  message   : Message
  timestamp : Timestamp
}

enum GearPosition {
  PARK    = 0
  DRIVE   = 1
  REVERSE = 2
  NEUTRAL = 3
}

enumset WarningFlags {
  LOW_FUEL     = 0
  CHECK_ENGINE = 1
  DOOR_OPEN    = 2
  SEATBELT     = 3
}

/**
 * Main vehicle status interface.
 *
 * Publishes real-time vehicle state and exposes control
 * commands for gear and speed management.
 *
 * ```ridl
 * import veh.cluster.VehicleStatus
 * ```
 *
 * @see veh.powertrain.PowertrainInterface
 * @labels SIL_B, CAL_2, PRIVATE
 */
interface VehicleStatus {

  /// Current vehicle speed — strict periodic
  signal currentSpeed: Speed @10ms

  /// Engine temperature — change-driven with refresh ceiling
  signal engineTemp: Temperature @[20ms..100ms]

  /// Active warning flags — change-driven
  signal warnings: WarningFlags @[50ms..1s]

  /// Fired when the active speed limit is exceeded
  event speedLimitExceeded: SpeedLimitPayload @[100ms..2000ms]

  /// Fired when any door state changes
  event doorOpened: DoorPayload @[50ms..500ms]

  /// Request a gear change
  command setGear(position: GearPosition) [
    require position != GearPosition.PARK || currentSpeed == 0.0
  ]

  /// Set a speed range
  command setRange(min: Speed, max: Speed) [
    require min < max
    require max <= MAX_SPEED
  ]

  /// Reset all active fault codes
  command resetFaults()

  /// Compute average speed over a sliding window
  query getAverageSpeed(window: Duration): Speed [
    require window > 0ms
    ensure  result >= 0.0
  ]

  /// Get speed bounds over a window
  query getMinMax(window: Duration): (min: Speed, max: Speed) [
    require window > 0ms
    ensure  result.min <= result.max
  ]

  /**
   * Stream active faults matching a filter.
   * @deprecated "use streamFaultsV2 instead"
   */
  query streamFaults(filter: DiagFilter): <FaultEvent>

  // provisioned at build or FOTA
  final softwareVersion: Version
  final ecuSerial: Uuid
  final capabilities: [Label; 0..32]
}
````

---

## Appendix C — Standards References

| Standard                                                           | Used for                              |
| ------------------------------------------------------------------ | ------------------------------------- |
| [Unicode 15.0](https://www.unicode.org/versions/Unicode15.0.0/)    | Source file encoding                  |
| [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629)                 | UTF-8 encoding                        |
| [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)                 | String escape sequences               |
| [ECMA-262](https://tc39.es/ecma262/)                               | Regex literal syntax                  |
| [CommonMark](https://commonmark.org)                               | Doc comment markdown                  |
| [IEEE 754-2019](https://ieeexplore.ieee.org/document/8766229)      | Floating-point representation         |
| [ISO/IEC 9899:2018 (C17)](https://www.iso.org/standard/74528.html) | Integer and float literal conventions |
| [UCUM](https://ucum.org/ucum)                                      | Physical unit expressions             |
| [ISO 8601](https://www.iso.org/standard/70907.html)                | Timestamp, Date, TimeOfDay types      |
| [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339)                 | Timestamp format                      |
| [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986)                 | URI type                              |
| [RFC 4122](https://www.rfc-editor.org/rfc/rfc4122)                 | UUID type                             |
| [SemVer 2.0.0](https://semver.org/)                                | Version type                          |
| [ISO 3779](https://www.iso.org/standard/52200.html)                | VIN format                            |
| [ISO 3166-1](https://www.iso.org/standard/72482.html)              | CountryCode type                      |
| [ISO 639-1](https://www.iso.org/standard/22109.html)               | LanguageCode type                     |
| [AUTOSAR Classic R22-11](https://www.autosar.org/)                 | Interface modeling patterns           |
| [Kotlin Language Spec](https://kotlinlang.org/spec/)               | Package, import, syntax conventions   |
| [KDoc / Dokka](https://kotlinlang.org/docs/kotlin-doc.html)        | Doc comment conventions               |
| [Rustdoc](https://doc.rust-lang.org/rustdoc/)                      | Doc comment conventions               |
| [Rust Reference](https://doc.rust-lang.org/reference/)             | Fixed array syntax, primitive naming  |

---

## Appendix D — Codegen Targets

### Width Mapping Strategy

The RIDL compiler resolves canonical wire widths once — passed to all codegens,
never inferred independently per target.

**Language layer** — always widest, no overflow risk:

| Canonical   | Rust  | Kotlin   | C++       |
| ----------- | ----- | -------- | --------- |
| any integer | `i64` | `Long`   | `int64_t` |
| any float   | `f64` | `Double` | `double`  |
| enumset     | `i64` | `Long`   | `int64_t` |

**Transport layer** — narrowest that safely fits the declared range:

| Canonical | proto3    | FlatBuffers | SOME/IP   | CAN            | AIDL     |
| --------- | --------- | ----------- | --------- | -------------- | -------- |
| `uint8`   | `uint32`* | `uint8`     | `UINT8`   | 8 bits         | `int`    |
| `uint16`  | `uint32`* | `uint16`    | `UINT16`  | 16 bits        | `int`    |
| `uint32`  | `uint32`  | `uint32`    | `UINT32`  | 32 bits        | `int`    |
| `int32`   | `int32`   | `int32`     | `SINT32`  | 32 bits signed | `int`    |
| `int64`   | `int64`   | `int64`     | `SINT64`  | 64 bits signed | `long`   |
| `float32` | `float`   | `float32`   | `FLOAT`   | 32 bits        | `float`  |
| `float64` | `double`  | `float64`   | `FLOAT64` | 64 bits        | `double` |

*proto3 has no `uint8`/`uint16` — uses `uint32` with varint encoding (small
values use 1-2 bytes on wire).

### Target List

| Target                    | Role          | Notes                                                                                        |
| ------------------------- | ------------- | -------------------------------------------------------------------------------------------- |
| Rust                      | language      | `i64`, `f64`, `bitflags` for enumset                                                         |
| Kotlin                    | language      | `Long`, `Double`, `EnumSet` for enumset                                                      |
| C++                       | language      | `int64_t`, `double`, bitmask for enumset                                                     |
| Protocol Buffers (proto3) | transport     | [developers.google.com/protocol-buffers](https://developers.google.com/protocol-buffers)     |
| FlatBuffers               | transport     | [flatbuffers.dev](https://flatbuffers.dev/)                                                  |
| SOME/IP                   | transport     | [AUTOSAR SOME/IP](https://www.autosar.org/)                                                  |
| CAN / DBC                 | transport     | signal bit-packing per declared range                                                        |
| AIDL                      | transport     | [Android Interface Definition Language](https://developer.android.com/guide/components/aidl) |
| UML / SysML               | documentation | interface and sequence diagrams                                                              |

---

## Appendix E — Formal Grammar (EBNF)

```ebnf
(* Top-level *)
file          = package { import } { definition } ;

package       = "package" qualified_id ;

import        = "import" qualified_id [ "as" id ]
              | "import" qualified_id ".*"
              ;

definition    = type_def
              | const_def
              | struct_def
              | enum_def
              | enumset_def
              | union_def
              | interface_def
              ;

(* ---------- Type ---------- *)

type_def      = doc_comment? "type" id ":" type_backing constraint? ;

type_backing  = "integer" | "float" | "string" | "bytes" | ucum_unit ;

ucum_unit     = (* UCUM expression: km/h, Cel, N.m, /min, %, bar, V, A ... *)

constraint    = "[" constraint_spec "]" ;

constraint_spec
              = scalar ".." scalar ( "step" scalar )?
              | scalar ".."
              | ".." scalar
              | int_lit
              | int_lit ".." int_lit
              | int_lit "match" pattern
              | int_lit ".." int_lit "match" pattern
              | "match" pattern
              ;

pattern       = regex_lit | SCREAMING_SNAKE_ID ;
scalar        = int_lit | float_lit ;

(* ---------- Constants ---------- *)

const_def     = doc_comment? "const" SCREAMING_SNAKE_ID ( ":" id )? "=" ( literal | regex_lit ) ;

(* ---------- Struct ---------- *)

struct_def    = doc_comment? "struct" id "{" { field sep? } "}" ;
field         = doc_comment? id ":" field_type ;

field_type    = primitive_type
              | id
              | field_type "?"
              | "[" field_type ";" int_lit "]"
              | "[" field_type ";" int_lit ".." int_lit "]"
              | "[" ( primitive_type | id ) ":" field_type ";" int_lit ".." int_lit "]"
              | "<" ( field_type | "string" | "bytes" ) ">"
              ;

primitive_type = "boolean" | "integer" | "float" | "string" | "bytes" ;

(* ---------- Enum ---------- *)

enum_def      = doc_comment? "enum" id "{" { enum_value sep? } "}" ;
enum_value    = doc_comment? SCREAMING_SNAKE_ID "=" int_lit ;

(* ---------- EnumSet ---------- *)

enumset_def   = doc_comment? "enumset" id "{" { enumset_bit sep? } "}"
              | doc_comment? "enumset" id ":" id
              ;
enumset_bit   = doc_comment? SCREAMING_SNAKE_ID "=" int_lit ;

(* ---------- Union ---------- *)

union_def     = doc_comment? "union" id "{" { union_arm sep? } "}" ;
union_arm     = doc_comment? id ":" id ;

(* ---------- Interface ---------- *)

interface_def = doc_comment? "interface" id "{" { interaction } "}" ;
interaction   = signal_def | event_def | command_def | query_def | final_def ;

signal_def    = doc_comment? "signal"  id ":" id timing attr_block? ;
event_def     = doc_comment? "event"   id ":" id timing attr_block? ;
command_def   = doc_comment? "command" id "(" param_list ")" attr_block? ;
query_def     = doc_comment? "query"   id "(" param_list ")" ":" return_type attr_block? ;
final_def     = doc_comment? "final"   id ":" field_type ;

param_list    = "" | param { "," param } ;
param         = id ":" field_type ;

return_type   = id
              | "(" named_field { "," named_field } ")"
              ;

named_field   = id ":" field_type ;

(* ---------- Timing ---------- *)

timing        = "@" duration
              | "@" "[" timing_range "]"
              ;

timing_range  = duration ".." duration
              | duration ".."
              | ".." duration
              ;

duration      = int_lit ( "us" | "ms" | "s" ) ;

(* ---------- Attribute Block ---------- *)

attr_block    = "[" { attribute sep? } "]" ;

attribute     = "default" "=" literal
              | "require" expr
              | "ensure"  expr
              ;

(* ---------- Doc Comments ---------- *)

doc_comment   = "/**" { doc_tag | markdown_text } "*/"
              | { "///" markdown_text newline }
              ;

doc_tag       = "@see"        qualified_id
              | "@labels"     label { "," label }
              | "@deprecated" string_lit
              ;

label         = SCREAMING_SNAKE_ID [ "(" SCREAMING_SNAKE_ID ")" ]
              (* free-form — vocabulary validated by active profile, not by the compiler *)
              ;

(* ---------- Separators ---------- *)

sep           = "," | newline ;

(* ---------- Identifiers and Literals ---------- *)

qualified_id       = id { "." id } ;
id                 = CamelCase_id | camelCase_id ;
CamelCase_id       = [A-Z][a-zA-Z0-9]* ;
camelCase_id       = [a-z][a-zA-Z0-9]* ;
SCREAMING_SNAKE_ID = [A-Z][A-Z0-9_]* ;

int_lit       = [0-9]+ ;
float_lit     = [0-9]+ "." [0-9]+ ;
string_lit    = '"' { utf8_char } '"' ;
regex_lit     = "/" { regex_char } "/" ;
bool_lit      = "true" | "false" ;
literal       = int_lit | float_lit | string_lit | bool_lit ;
```
