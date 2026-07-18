# Getting Started with RIDL

**Reactive Interface Description Language** — a practical introduction for
automotive software engineers.

---

## What is RIDL?

RIDL is a language for describing the contracts between software components in a
vehicle. It answers the question: _what does this service produce, consume, and
guarantee?_

A RIDL file describes an **interface** — not an implementation. It does not say
how a service works internally, only what it exposes to the outside world. This
makes it the single source of truth between teams, components, and tools.

RIDL is transport-neutral. The same interface definition generates code for
protobuf, AIDL, SOME/IP, FlatBuffers, or CAN. The contract is defined once,
deployed anywhere.

---

## Your First Interface

Create a file called `speed.ridl`:

```ridl
package veh.cluster

import veh.common.Speed

/// Vehicle speed reporting interface
interface VehicleSpeed {

  /// Current vehicle speed, published every 10ms
  signal currentSpeed: Speed @10ms
}
```

- `package veh.cluster` — this file belongs to the `veh.cluster` package
- `import veh.common.Speed` — `Speed` is defined in another package
- `interface VehicleSpeed` — the contract boundary
- `signal currentSpeed: Speed @10ms` — a continuously published value at 10ms

---

## Primitives and Types

RIDL has five primitives: `boolean`, `integer`, `float`, `string`, `bytes`.
Primitives are always used through named domain types. `string` and `bytes`
default to `[0..256]` when unspecified — active profiles may require explicit
bounds. Collection bounds are always explicit — no defaults.

```ridl
package veh.common

// physical unit types — UCUM units
type Speed       : km/h  [0.0..250.0 step 0.5]
type Temperature : Cel   [-40.0..125.0 step 0.1]
type FuelLevel   : %     [0.0..100.0 step 0.1]
type Voltage     : V     [0.0..48.0 step 0.1]

// constrained integer types
type Counter : integer [0..65535]
type Index   : integer [0..255]

// string types — bounds always required
type ModelCode : string [3..6 match MODEL_PATTERN]
type Notes     : string [1..256]
```

The range constraint informs the compiler — `integer [0..255]` becomes `uint8`
on transport, `int64` in generated language code. No explicit width types
needed.

Stdlib types like `Uuid`, `Vin`, `Timestamp`, `Version`, `Label`, `Message` are
always available without import.

---

## Constants

Compile-time constants reusable across types, constraints, and contracts:

```ridl
package veh.common

const MAX_SPEED      : Speed   = 250.0
const SPEED_LIMIT_EU : Speed   = 130.0
const MAX_GEAR       : integer = 6

// regex constants
const MODEL_PATTERN = /^[A-Z][A-Z0-9]{2,5}$/
```

Reused in types, structs, and contracts:

```ridl
type Speed     : km/h [0.0..MAX_SPEED step 0.5]
type ModelCode : string [3..6 match MODEL_PATTERN]

struct GearState {
  gear : integer [0..MAX_GEAR]
}

command setTargetSpeed(speed: Speed) [
  require speed <= SPEED_LIMIT_EU
]
```

---

## Signals and Events

**Signal** — a continuously sampled value. The latest sample matters.

```ridl
signal currentSpeed: Speed @10ms              // strict periodic — every 10ms
signal engineTemp: Temperature @[20ms..100ms] // change-driven, refresh every 100ms at most
signal fuelLevel: FuelLevel @[..5s]           // refresh ceiling only
```

**Event** — a discrete occurrence. Every occurrence matters.

```ridl
event collisionDetected: CollisionPayload @[100ms..2000ms]
// 100ms — throttle: min interval between events
// 2000ms — TTL: discard if processed more than 2000ms after raised
```

---

## Enum Sets

For multiple simultaneous flags use `enumset` — a named bitfield:

```ridl
enumset WarningFlags {
  LOW_FUEL     = 0    // bit 0
  CHECK_ENGINE = 1    // bit 1
  DOOR_OPEN    = 2    // bit 2
  SEATBELT     = 3    // bit 3
}

signal activeWarnings: WarningFlags @[50ms..1s]
```

Or derive from an existing enum when you need both single and multi-value use:

```ridl
enum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1, DOOR_OPEN = 2, SEATBELT = 3 }
enumset WarningFlags : Warning

command clearWarning(flag: Warning)        // single flag
signal activeWarnings: WarningFlags @50ms  // multiple flags
```

---

## Provisioned Values — Final

Values set at build, factory, or FOTA — immutable for the software instance
lifetime:

```ridl
interface VehicleIdentity {
  final vin: Vin
  final softwareVersion: Version
  final marketRegion: Label
  final ecuSerial: Uuid
  final capabilities: [Label; 0..32]
}
```

`final` values are safe to cache — they never change while the software is
running.

---

## Commands and Queries

**Command** — fire-and-forget. No return value.

```ridl
command setTargetSpeed(speed: Speed)
command enableCruiseControl()
command resetFaults()
```

**Query** — request/response. Always returns a value.

```ridl
query getCurrentSpeed(): Speed
query getSpeedHistory(window: Duration): (min: Speed, max: Speed, avg: Speed)
```

---

## Structs

```ridl
enum DriveMode { NORMAL = 0, ECO = 1, SPORT = 2, OFF_ROAD = 3 }

struct CruiseControlState {
  enabled     : boolean
  targetSpeed : Speed
  mode        : DriveMode
  override    : Speed?     // optional — absent if no override active
}

signal cruiseState: CruiseControlState @[50ms..500ms]
```

---

## Collections and Streams

Collections are finite and always bounded:

```ridl
struct DiagReport {
  faults   : [FaultCode; 0..32]       // bounded array — max 32 faults
  sensors  : [Label : Speed; 1..8]    // bounded map — 1 to 8 entries
  rawFrame : bytes [64]               // fixed 64-byte buffer
}
```

Streams are unbounded — transport decides framing. Element type is a named type:

```ridl
type LogLine : string [1..1024]     // one logical log entry
type FwBlock : bytes [1..65536]    // one logical firmware block

query streamFaults(filter: DiagFilter): <FaultEvent>   // structured stream
query streamLogs(level: AlertLevel): <LogLine>          // text stream
command uploadFirmware(data: <FwBlock>)                 // binary stream
query pipe(input: <SensorSample>): <ProcessedSample>   // bidirectional
```

---

## Contracts

```ridl
command setTargetSpeed(speed: Speed) [
  require speed > 0.0
  require speed <= SPEED_LIMIT_EU
]

query getSpeedHistory(window: Duration): (min: Speed, max: Speed, avg: Speed) [
  require window > 0ms
  ensure  result.min <= result.avg
  ensure  result.avg <= result.max
]
```

---

## Labels

`@labels` carries free-form classification identifiers — safety, security,
privacy, or any other domain-specific tag. The core language defines no
vocabulary. Labels are validated by an external **profile** attached to `ridlc`.

```ridl
/**
 * Cruise control interface.
 * @labels SIL_2, SEC_2, PRIVATE
 */
interface CruiseControl { ... }

/**
 * Logging interface — no safety or security requirement.
 * @labels SIL_QM, SEC_NA, PUBLIC
 */
interface LogService { ... }
```

Different domains use different vocabularies — an automotive profile defines
`ASIL_*` and `CAL_*`, a medical profile defines `IEC_62304_*`, an industrial
profile defines `PLd` or `IEC_61508_*`. RIDL passes labels through unchanged;
the active profile validates them.

---

## Next Steps

- Read the [RIDL Language Reference](ridl-language-reference.md) for the full
  specification
- See the sample interfaces in the annexes below for real-world patterns

---

## Annex 1 — Vehicle Identity Interface

A pure `final` interface — fully provisioned at factory or FOTA.

```ridl
package veh.identity

enum FuelType   { PETROL = 0, DIESEL = 1, HYBRID = 2, ELECTRIC = 3, HYDROGEN = 4 }
enum DriveLayout { FWD = 0, RWD = 1, AWD = 2, FourWD = 3 }

/**
 * Vehicle identity and hardware capability manifest.
 * Provisioned at factory. Software fields updated via FOTA.
 *
 * @labels SIL_QM, SEC_2, CONFIDENTIAL
 */
interface VehicleIdentity {

  // vehicle identity — factory provisioned
  final vin          : Vin
  final modelYear    : integer [2000..2100]
  final marketRegion : Label
  final fuelType     : FuelType
  final driveLayout  : DriveLayout

  // software identity — FOTA updated
  final softwareVersion   : Version
  final bootloaderVersion : Version
  final hardwareVersion   : Version

  // capability flags — build time
  final hasAdaptiveCruise   : boolean
  final hasEmergencyBraking : boolean
  final hasLaneKeepAssist   : boolean
  final hasParkingAssist    : boolean
  final hasDriverMonitoring : boolean
  final maxSupportedSpeed   : Speed
}
```

---

## Annex 2 — Powertrain Interface

```ridl
package veh.powertrain

type RPM         : /min  [0.0..8000.0 step 10.0]
type Torque      : N.m   [0.0..500.0 step 0.1]
type Temperature : Cel   [-40.0..150.0 step 0.1]
type FuelLevel   : %     [0.0..100.0 step 0.1]
type Voltage     : V     [0.0..48.0 step 0.1]

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
  state       : EngineState
}

struct TransmissionState {
  gear      : GearPosition
  inputRPM  : RPM
  outputRPM : RPM
  oilTemp   : Temperature
}

struct FaultPayload {
  dtc       : integer [0..65535]    // Diagnostic Trouble Code
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
  signal transmissionState : TransmissionState @[50ms..500ms]
  signal fuelLevel         : FuelLevel @[..5s]
  signal batteryVoltage    : Voltage @[100ms..1s]

  event engineFault      : FaultPayload @[100ms..30s]
  event transmissionFault: FaultPayload @[100ms..30s]

  command requestStart(mode: EngineState) [
    require mode == EngineState.CRANKING
  ]
  command requestStop()
  command setGear(gear: GearPosition) [
    require gear != GearPosition.PARK || engineMetrics.rpm == 0.0
  ]

  query getDiagnostics(): (
    engineState   : EngineState
    activeGear    : GearPosition
    faultCount    : integer [0..65535]
    lastFaultTime : Timestamp?
  )

  query streamFaults(severity: integer [0..5]): <FaultPayload>

  final softwareVersion    : Version
  final calibrationVersion : Version
  final ecuSerial          : Uuid
  final supportedGears     : [GearPosition; 1..8]
}
```

---

## Annex 3 — Body Control Interface

```ridl
package veh.body

type Ratio : % [0.0..100.0 step 1.0]

enum DoorPosition { CLOSED = 0, OPEN = 1, AJAR = 2 }
enum LockState    { LOCKED = 0, UNLOCKED = 1 }
enum LightState   { OFF = 0, ON = 1, FLASH = 2 }

struct DoorState {
  position : DoorPosition
  locked   : LockState
  sensorId : integer [0..7]
}

struct WindowState {
  position : Ratio
  sensorId : integer [0..7]
}

struct ClimateState {
  interiorTemp : veh.common.Temperature
  targetTemp   : veh.common.Temperature
  fanSpeed     : Ratio
  acActive     : boolean
}

struct AccessEvent {
  zone      : integer [0..7]
  granted   : boolean
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

  signal doorStates   : [DoorState; 1..8] @[50ms..2s]
  signal windowStates : [WindowState; 1..8] @[100ms..5s]
  signal climateState : ClimateState @[500ms..10s]

  event doorStateChanged : DoorState @[50ms..10s]

  /**
   * Vehicle access attempt — elevated privacy.
   * @labels SIL_1, SEC_4, CONFIDENTIAL
   */
  event accessAttempt : AccessEvent @[100ms..30s]

  command setAllLocks(state: LockState)
  command setDoorLock(sensorId: integer [0..7], state: LockState)
  command setWindowPosition(sensorId: integer [0..7], position: Ratio)
  command setClimateTarget(temp: veh.common.Temperature) [
    require temp >= 16.0
    require temp <= 30.0
  ]

  query getLockStatus(): (allLocked: boolean, unlockedCount: integer [0..8])
  query getBodyStatus(): (
    doors   : [DoorState; 1..8]
    windows : [WindowState; 1..8]
  )

  final doorCount          : integer [1..8]
  final windowCount        : integer [0..8]
  final hasElectricWindows : boolean
}
```

---

## Annex 4 — Driver Monitoring Interface

```ridl
package veh.dms

type Ratio : % [0.0..100.0 step 0.1]

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

struct DrownsinessState {
  level         : AlertLevel
  score         : Ratio
  eyeClosurePct : Ratio
}

struct DistractionEvent {
  level     : AlertLevel
  gazeZone  : GazeZone
  duration  : Duration
  timestamp : Timestamp
}

struct FatigueEvent {
  level     : AlertLevel
  score     : Ratio
  timestamp : Timestamp
}

/**
 * Driver Monitoring System interface.
 * Processes biometric and behavioral signals to assess driver state.
 *
 * @labels SIL_2, SEC_3, CONFIDENTIAL
 */
interface DriverMonitoring {

  signal attentionMetrics : AttentionMetrics @[100ms..500ms]
  signal drowsinessState  : DrownsinessState @[500ms..5s]

  event distractionDetected : DistractionEvent @[200ms..10s]
  event fatigueDetected     : FatigueEvent @[500ms..30s]

  command acknowledgeAlert(level: AlertLevel)
  command setMonitoringEnabled(enabled: boolean)

  query getDriverState(): (
    attentionScore  : Ratio
    drowsinessLevel : AlertLevel
    isDistracted    : boolean
    isFatigued      : boolean
  ) [
    ensure result.attentionScore >= 0.0
    ensure result.attentionScore <= 100.0
  ]

  query streamAttention(intervalMs: integer [50..1000]): <AttentionMetrics>

  final modelVersion   : Version
  final sensorType     : Label
  final gazeZoneCount  : integer [1..16]
}
```
