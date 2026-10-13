# typl

typl is the vocabulary layer: types, ranges, units, constants, and the
composites built from them. ridl, rmdl and rsdl reference typl vocabulary; typl
never references them.

## Primitives and named types

See
[Primitives](https://driftsys.github.io/ridl/reference/typl.html#4-primitives)
and
[Type Definitions](https://driftsys.github.io/ridl/reference/typl.html#5-type-definitions).

Five primitives: `boolean`, `integer`, `float`, `string`, `bytes`. In an
interaction — a signal payload, an event payload, a command or query parameter,
a `fixed` value — a primitive is never written directly; name a `type`. A struct
field is more permissive: an inline constrained primitive, such as a bounded
`string` or a bounded `bytes`, is accepted, but a bare `string` or `bytes` with
no constraint is `TYPL-208` in a struct field.

```typl,allow=TYPL-115
package skill.typl.primitives

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// A counter with no physical unit.
type Counter: integer [0..65535]

/// A unit-interval gain.
type Gain: float [0.0..1.0 step 0.01]

/// A vehicle identification number.
type Vin: string [17 match VIN_PATTERN] = "1HGCM82633A004352"

/// A fixed 8-byte frame.
type Frame: bytes [8]

/// A named boolean, so it can be used on a boundary.
type Enabled: boolean

/// The VIN pattern.
const VIN_PATTERN = /^[A-HJ-NPR-Z0-9]{17}$/
```

`Frame` draws `TYPL-115`: an 8-byte fixed buffer has no derivable init value and
declares none. The note is harmless wherever the type is never a signal payload
or a `fixed` value with no override; it becomes `RIDL-109` only there.

## Units and ranges

See
[Physical Unit Types](https://driftsys.github.io/ridl/reference/typl.html#51-physical-unit-types)
and
[Range Bounds](https://driftsys.github.io/ridl/reference/typl.html#55-range-bounds).

A unit type names a UCUM unit (`km/h`, `Cel`, `V`, `%`, `N.m`, `bar`, `/min`)
and a closed, inclusive range. `step` quantizes the range; the compiler derives
the wire width from the range, never from a width you write. Two differently
named types with the same unit and range do not interchange: typl typing is
nominal, so `Speed` and `Torque` are never assignable to each other even if
their ranges happen to match.

```typl
package skill.typl.unitsranges

/// Engine temperature, in degrees Celsius.
type EngineTemperature: Cel [-40.0..125.0 step 0.1]

/// A voltage reading.
type Voltage: V [0.0..48.0 step 0.1]

/// A fuel level as a percentage.
type FuelLevel: % [0.0..100.0 step 0.1]
```

## Constants

See
[Constants](https://driftsys.github.io/ridl/reference/typl.html#6-constants).

`const` declares a compile-time value, reusable in range bounds, `match`
patterns, init values, and (at ridl's interaction layer) contract expressions. A
regex `const` is written as a bare pattern literal, with no declared type.

```typl
package skill.typl.constants

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// The regulatory top of the speed range, reused in a range bound below.
const MAX_SPEED: Speed = 250.0

/// A model code pattern.
const MODEL_PATTERN = /^[A-Z][A-Z0-9]{2,5}$/

/// A model code, matched against the pattern above.
type ModelCode: string [3..6 match MODEL_PATTERN] = "ABC"

/// Engine speed, whose range reuses a constant.
type EngineSpeed: km/h [0.0..MAX_SPEED step 0.5]
```

## Composites — struct, enum, enumset, union, collections

See [Structs](https://driftsys.github.io/ridl/reference/typl.html#7-structs),
[Enums](https://driftsys.github.io/ridl/reference/typl.html#8-enums),
[Enum Sets](https://driftsys.github.io/ridl/reference/typl.html#9-enum-sets),
[Unions](https://driftsys.github.io/ridl/reference/typl.html#10-unions) and
[Collections](https://driftsys.github.io/ridl/reference/typl.html#12-collections).

A `struct` is a closed set of named fields; `?` on a field type means the field
may be absent, which is not the same as present-and-null — typl has no null. An
`enum` is an integer-backed set of discrete values, every value explicitly
numbered. An `enumset` is a named bitfield, standalone or derived from an
`enum`. A `union` is a tagged union whose arms reference named types only — a
primitive arm is `TYPL-204`. A collection is always explicitly bounded: an array
`[T; min..max]`, a fixed array `[T; N]`, or a map `[K: V; min..max]`. No
composite may be recursive, directly or transitively (`TYPL-206`): an unbounded
wire size would contradict the bounded-size guarantee every typl composite
carries.

```typl
package skill.typl.composites

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// The selected drive mode.
enum DriveMode {
  NORMAL = 0
  ECO = 1
  SPORT = 2
}

/// Every drive mode active at once -- derived from `DriveMode`.
enumset DriveModeFlags: DriveMode

/// The state of cruise control.
struct CruiseControlState {
  enabled: boolean
  targetSpeed: Speed
  mode: DriveMode
  override: Speed? // optional -- absent when no override is active
}

/// A shape reported by a collision sensor.
union Shape {
  circle: Circle
  square: Square
}

/// A circular shape.
struct Circle {
  radius: float [0.0..100.0 step 0.01]
}

/// A rectangular shape.
struct Square {
  side: float [0.0..100.0 step 0.01]
  depth: float [0.0..100.0 step 0.01]
}

/// A log of faults, with a bounded array, a bounded map and a fixed buffer.
struct FaultLog {
  faults: [integer [0..65535]; 0..32]
  levels: [Label: Speed; 1..8]
  frame: bytes [8]
}
```

## Errors as data — the result union

See
[Error Types](https://driftsys.github.io/ridl/reference/typl.html#101-error-types)
and
[Result Unions](https://driftsys.github.io/ridl/reference/typl.html#102-result-unions).

typl has no exceptions and no `throws`. The `error` modifier marks an `enum`,
`struct` or `union` as failure vocabulary (`TYPL-212`); an `error union`'s arms
must all themselves be error-typed (`TYPL-214`). A **result union** is a `union`
mixing exactly one non-error arm and one error arm — this is the family's entire
functional-error mechanism, achieved with no generics and no `Result<T, E>`
built-in. A union that mixes the two kinds any other way is `TYPL-213`.

```typl
package skill.typl.errors

/// Calibration failure vocabulary.
error enum CalError {
  SENSOR_UNAVAILABLE = 0
  VEHICLE_MOVING = 1
}

/// A successful calibration report.
struct CalReport {
  offset: float [-1.0..1.0 step 0.001]
  axle: integer [0..3]
}

/// The result of a calibration attempt.
union CalOutcome {
  ok: CalReport
  err: CalError
}
```

## Packages, imports and visibility

See
[Packages, Imports, and Visibility](https://driftsys.github.io/ridl/reference/typl.html#3-packages-imports-and-visibility).

One `package` declaration per file, dot-separated lowercase, mirroring the
directory below the manifest root (a mismatch is `TYPL-002`). Imports are
single-type and qualified, with an optional `as` alias for a genuine collision;
wildcards, relative imports and re-exports are all `TYPL-003`. `internal` marks
a declaration package-private; a public declaration must not expose an
`internal` type in a field, an arm, or a bound constant (`TYPL-005`). The
lattice runs one way, `typl ← {ridl, rmdl} ← rsdl`: nothing in a `.typl` file
resolves a name declared only in a `.ridl`, `.rmdl` or `.rsdl` file
(`TYPL-011`).

```typl
package skill.typl.pkgs.common

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// A raw sensor sample, for the use of this package only.
internal struct RawSample {
  value: Speed
}
```

```typl
package skill.typl.pkgs.consumer

import skill.typl.pkgs.common.Speed

/// A display reading built from the shared vocabulary.
struct DisplayReading {
  speed: Speed
}
```

## Evolution of a type

See
[Field Identity and Evolution](https://driftsys.github.io/ridl/reference/typl.html#74-field-identity-and-evolution).

A struct field's identity is its declaration order, so evolution is append-only:
add a new field at the end, and retire a removed one with a `reserved` tombstone
rather than deleting or reusing its slot (re-declaring under a reserved name or
value is `TYPL-210`; a duplicate `reserved` entry is `TYPL-211`). An `enum` or
`enumset` value's identity is its explicit number, not its position, so
reordering one is compatible; a reserved name retires a name and a reserved
value retires a number, and both forms stay legal in an `enum` body.

```typl
package skill.typl.evolution

/// A driver profile, after one field was retired.
struct DriverProfile {
  name: Name
  reserved legacyChecksum // was field 1 -- retired, never reused
  speed: Speed
}

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// A gear position, after one value was retired.
enum GearPosition {
  PARK = 0
  reserved 1
  DRIVE = 2
}
```

## Common mistakes

See
[Diagnostics](https://driftsys.github.io/ridl/reference/typl.html#16-diagnostics).

| Mistake                                          | Correct form                                         | Code       |
| ------------------------------------------------ | ---------------------------------------------------- | ---------- |
| `integer` with no range                          | `integer [min..max]`                                 | `TYPL-101` |
| `float` with no range and `step`                 | `float [min..max step s]`                            | `TYPL-102` |
| `string`/`bytes` with no bounds                  | `string [min..max]` or `bytes [min..max]`            | `TYPL-103` |
| a bare `string`/`bytes` as a field type          | define a named `type`, or constrain it inline        | `TYPL-208` |
| an array with no bounds                          | `[T; min..max]`                                      | `TYPL-201` |
| a map with no bounds                             | `[K: V; min..max]`                                   | `TYPL-202` |
| a union arm of a primitive type                  | arms reference named types only                      | `TYPL-204` |
| a recursive struct, enum, or union               | model with a bounded array or map of indices         | `TYPL-206` |
| reordering or reusing a struct field             | append new fields at the end; `reserved` the rest    | `TYPL-210` |
| a wildcard, relative, or re-exporting import     | a single qualified `import pkg.Name [as Alias]`      | `TYPL-003` |
| a public declaration exposing an `internal` type | re-declare the type as public, or keep both internal | `TYPL-005` |
